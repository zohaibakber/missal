import { Buffer } from "node:buffer";

export const PRINT_IDLE_CLOSE_MS = 2 * 60_000;

const PRINT_BUSY_MESSAGE =
  "Missal is already preparing a print. Wait for it to finish, then try again.";

const PRINT_TOO_LARGE_MESSAGE = "This document is too large to prepare for print.";

type PrintPhase = "pageLoad" | "layout" | "fonts" | "images" | "pdf" | "print";

/** Jobs waiting for the hidden window, not counting the one using it. */
const MAX_QUEUED_JOBS = 4;
/** UTF-8 bytes of HTML held by waiting and running jobs. */
const MAX_RETAINED_HTML_BYTES = 48 * 1024 * 1024;

const DEADLINES: { readonly [Phase in PrintPhase]: number } = {
  pageLoad: 15_000,
  layout: 15_000,
  fonts: 10_000,
  images: 20_000,
  pdf: 60_000,
  // The system print dialog waits on the user, so this only bounds a lost callback.
  print: 5 * 60_000,
};

export type PrintOwner = {
  readonly webContentsId: number;
  readonly ownerId: string;
};

export type PrintJobRequest = {
  readonly kind: "preview" | "print";
  readonly owner: PrintOwner;
  readonly requestId: string;
  readonly origin: string;
  readonly html: string;
};

type PrintedJob = {
  readonly printed: boolean;
  readonly failureReason?: string;
};

export type PrintJobResult =
  | { readonly _tag: "Pdf"; readonly pdf: Uint8Array }
  | ({ readonly _tag: "Printed" } & PrintedJob)
  | { readonly _tag: "Busy"; readonly message: string }
  | { readonly _tag: "TimedOut"; readonly phase: PrintPhase; readonly message: string }
  | { readonly _tag: "Failed"; readonly message: string }
  | { readonly _tag: "Superseded" }
  | { readonly _tag: "Cancelled" };

export type PrintJobRunner = {
  runPhase: (
    phase: PrintPhase,
    job: PrintJobRequest,
    signal: AbortSignal,
  ) => Promise<void | Uint8Array | PrintedJob>;
  /** Destroys the shared window so no later job inherits a stuck or stale page. */
  resetWindow: () => void;
  noteIdle: () => void;
};

/** A failure whose message is safe to show the user. */
export class PrintFailure extends Error {}

class PrintTimeout extends PrintFailure {
  constructor(readonly phase: PrintPhase) {
    super(TIMEOUT_MESSAGES[phase]);
  }
}

const TIMEOUT_MESSAGES: { readonly [Phase in PrintPhase]: string } = {
  pageLoad: "The print view did not open in time. Try again.",
  layout: "The document did not finish laying out in time. Try again.",
  fonts: "The Urdu font did not become ready in time. Try again.",
  images: "An image in the document did not finish loading in time. Try again.",
  pdf: "Creating the preview took too long. Try again.",
  print: "Printing took too long and was stopped. Try again.",
};

const PHASES = {
  preview: ["pageLoad", "layout", "fonts", "images", "pdf"],
  print: ["pageLoad", "layout", "fonts", "images", "print"],
} as const satisfies Record<PrintJobRequest["kind"], readonly PrintPhase[]>;

type Job = PrintJobRequest & {
  readonly bytes: number;
  readonly resolve: (result: PrintJobResult) => void;
};

type StopReason = "Superseded" | "Cancelled";

const sameOwner = (job: Job, owner: PrintOwner) =>
  job.owner.webContentsId === owner.webContentsId && job.owner.ownerId === owner.ownerId;

const busy = (message = PRINT_BUSY_MESSAGE): Promise<PrintJobResult> =>
  Promise.resolve({ _tag: "Busy", message });

// Rejects with the abort reason or a timeout; the runner sees the same signal and stops too.
const withDeadline = <T>(
  operation: Promise<T>,
  phase: PrintPhase,
  ms: number,
  abort: AbortController,
) =>
  new Promise<T>((resolve, reject) => {
    const { signal } = abort;
    const onAbort = () => reject(signal.reason);
    const timer = setTimeout(() => abort.abort(new PrintTimeout(phase)), ms);
    signal.addEventListener("abort", onAbort, { once: true });
    operation.then(resolve, reject).finally(() => {
      clearTimeout(timer);
      signal.removeEventListener("abort", onAbort);
    });
  });

/**
 * Admits jobs for the one hidden print window. A newer preview replaces an older one from the same
 * owner; accepted prints keep their order and are never dropped; a job that does not fit is busy
 * immediately instead of waiting.
 */
export function createPrintService(runner: PrintJobRunner) {
  let queue: Job[] = [];
  let running: { readonly job: Job; readonly abort: AbortController } | undefined;
  let draining = false;

  const retainedBytes = () => queue.reduce((sum, job) => sum + job.bytes, running?.job.bytes ?? 0);

  const stopRunning = (reason: StopReason) => {
    if (!running) return;
    running.abort.abort(reason);
    running = undefined;
    runner.resetWindow();
  };

  async function execute(job: Job, abort: AbortController): Promise<PrintJobResult> {
    let output: void | Uint8Array | PrintedJob = undefined;
    for (const phase of PHASES[job.kind]) {
      abort.signal.throwIfAborted();
      output = await withDeadline(
        runner.runPhase(phase, job, abort.signal),
        phase,
        DEADLINES[phase],
        abort,
      );
    }
    abort.signal.throwIfAborted();
    if (job.kind === "preview") {
      return output instanceof Uint8Array
        ? { _tag: "Pdf", pdf: output }
        : { _tag: "Failed", message: "The preview PDF was empty." };
    }
    const printed = output instanceof Uint8Array ? undefined : output;
    return {
      _tag: "Printed",
      printed: printed?.printed ?? false,
      failureReason: printed?.failureReason,
    };
  }

  async function runOne(job: Job) {
    const abort = new AbortController();
    running = { job, abort };
    const result = await execute(job, abort).catch((error: unknown): PrintJobResult => {
      const reason: unknown = abort.signal.aborted ? abort.signal.reason : error;
      if (reason === "Superseded" || reason === "Cancelled") return { _tag: reason };
      if (reason instanceof PrintTimeout) {
        return { _tag: "TimedOut", phase: reason.phase, message: reason.message };
      }
      return {
        _tag: "Failed",
        message:
          reason instanceof PrintFailure ? reason.message : "The print view failed. Try again.",
      };
    });
    if (running?.job === job) running = undefined;
    if (result._tag === "Pdf" || result._tag === "Printed") runner.noteIdle();
    else runner.resetWindow();
    job.resolve(result);
  }

  async function drain() {
    if (draining) return;
    draining = true;
    for (let job = queue.shift(); job; job = queue.shift()) await runOne(job);
    draining = false;
  }

  const admit = (request: PrintJobRequest, bytes: number, place: (job: Job) => void) =>
    new Promise<PrintJobResult>((resolve) => {
      place({ ...request, bytes, resolve });
      void drain();
    });

  return {
    submit(request: PrintJobRequest): Promise<PrintJobResult> {
      const bytes = Buffer.byteLength(request.html, "utf8");
      if (bytes > MAX_RETAINED_HTML_BYTES) return busy(PRINT_TOO_LARGE_MESSAGE);

      if (request.kind === "preview") {
        const index = queue.findIndex(
          (job) => job.kind === "preview" && sameOwner(job, request.owner),
        );
        const previous = queue[index];
        if (previous) {
          if (retainedBytes() - previous.bytes + bytes > MAX_RETAINED_HTML_BYTES) return busy();
          return admit(request, bytes, (job) => {
            queue[index] = job;
            previous.resolve({ _tag: "Superseded" });
          });
        }
        if (running?.job.kind === "preview" && sameOwner(running.job, request.owner)) {
          stopRunning("Superseded");
        }
      }

      if (queue.length >= MAX_QUEUED_JOBS || retainedBytes() + bytes > MAX_RETAINED_HTML_BYTES) {
        return busy();
      }
      return admit(request, bytes, (job) => queue.push(job));
    },

    cancelPreview(owner: PrintOwner, requestId: string) {
      const matches = (job: Job) =>
        job.kind === "preview" && job.requestId === requestId && sameOwner(job, owner);
      const queued = queue.find(matches);
      if (queued) {
        queue = queue.filter((job) => job !== queued);
        queued.resolve({ _tag: "Cancelled" });
      } else if (running && matches(running.job)) {
        stopRunning("Cancelled");
      }
    },

    releaseWebContents(webContentsId: number) {
      const closing = queue.filter((job) => job.owner.webContentsId === webContentsId);
      queue = queue.filter((job) => job.owner.webContentsId !== webContentsId);
      for (const job of closing) job.resolve({ _tag: "Cancelled" });
      if (running?.job.owner.webContentsId === webContentsId) stopRunning("Cancelled");
    },
  };
}

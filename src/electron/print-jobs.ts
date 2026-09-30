import { Buffer } from "node:buffer";

/**
 * Admission for the one hidden print window.
 * A newer preview replaces an older preview from the same owner.
 * Accepted prints stay in submission order and are never dropped.
 * A request that does not fit fails immediately instead of waiting.
 */
export const PRINT_IDLE_CLOSE_MS = 2 * 60_000;

/** Jobs waiting for the shared window. The job using the window is not included. */
export const PRINT_MAX_QUEUED_JOBS = 4;

/**
 * UTF-8 bytes of waiting and in-flight HTML.
 * Large enough for one photo-heavy FIR to be previewed and printed together.
 */
export const PRINT_MAX_RETAINED_HTML_BYTES = 48 * 1024 * 1024;

export const PRINT_DEADLINES_MS = {
  pageLoad: 15_000,
  layout: 15_000,
  fonts: 10_000,
  images: 20_000,
  pdf: 60_000,
  // The system print dialog waits on the user, so this only bounds a lost callback.
  print: 5 * 60_000,
} as const;

export const PRINT_BUSY_MESSAGE =
  "Missal is already preparing a print. Wait for it to finish, then try again.";

export const PRINT_TOO_LARGE_MESSAGE = "This document is too large to prepare for print.";

export const PRINT_ABORT_SUPERSEDED = "superseded";
export const PRINT_ABORT_CANCELLED = "cancelled";

export type PrintPhase = keyof typeof PRINT_DEADLINES_MS;

export type PrintOwner = {
  readonly webContentsId: number;
  readonly ownerId: string;
};

export type PrintJobKind = "preview" | "print";

export type PrintJobRequest = {
  readonly kind: PrintJobKind;
  readonly owner: PrintOwner;
  readonly requestId: string;
  readonly origin: string;
  readonly html: string;
};

export type PrintedJob = {
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

export type PrintPhaseOutput = void | Uint8Array | PrintedJob;

export type RunningPrintJob = {
  readonly kind: PrintJobKind;
  readonly requestId: string;
  readonly origin: string;
  readonly html: string;
};

export type PrintJobRunner = {
  runPhase: (
    phase: PrintPhase,
    job: RunningPrintJob,
    signal: AbortSignal,
  ) => Promise<PrintPhaseOutput>;
  /**
   * Destroy the shared window before another job uses it.
   * In-flight work must not touch that window again; the service does not wait for it.
   */
  resetWindow: () => void;
  noteIdle: () => void;
};

export type PrintServiceLimits = {
  readonly maxQueuedJobs: number;
  readonly maxRetainedHtmlBytes: number;
  readonly deadlines: { readonly [Phase in PrintPhase]: number };
};

export const PRINT_SERVICE_LIMITS: PrintServiceLimits = {
  maxQueuedJobs: PRINT_MAX_QUEUED_JOBS,
  maxRetainedHtmlBytes: PRINT_MAX_RETAINED_HTML_BYTES,
  deadlines: PRINT_DEADLINES_MS,
};

export class PrintPhaseTimeoutError extends Error {
  readonly phase: PrintPhase;

  constructor(phase: PrintPhase) {
    super(timeoutMessage(phase));
    this.name = "PrintPhaseTimeoutError";
    this.phase = phase;
  }
}

class PrintAbortedError extends Error {
  readonly reason: unknown;

  constructor(reason: unknown) {
    super("Print job aborted");
    this.name = "PrintAbortedError";
    this.reason = reason;
  }
}

type InternalJob = {
  kind: PrintJobKind;
  owner: PrintOwner;
  requestId: string;
  origin: string;
  html: string;
  bytes: number;
  resolve: (result: PrintJobResult) => void;
};

const phasesFor = (kind: PrintJobKind): readonly PrintPhase[] =>
  kind === "preview"
    ? ["pageLoad", "layout", "fonts", "images", "pdf"]
    : ["pageLoad", "layout", "fonts", "images", "print"];

const sameOwner = (job: InternalJob, owner: PrintOwner) =>
  job.owner.webContentsId === owner.webContentsId && job.owner.ownerId === owner.ownerId;

const htmlBytes = (html: string) => Buffer.byteLength(html, "utf8");

const isPrintedJob = (value: PrintPhaseOutput): value is PrintedJob =>
  typeof value === "object" && value !== null && "printed" in value;

const timeoutMessage = (phase: PrintPhase) => {
  switch (phase) {
    case "pageLoad":
      return "The print view did not open in time. Try again.";
    case "layout":
      return "The document did not finish laying out in time. Try again.";
    case "fonts":
      return "The Urdu font did not become ready in time. Try again.";
    case "images":
      return "An image in the document did not finish loading in time. Try again.";
    case "pdf":
      return "Creating the preview took too long. Try again.";
    case "print":
      return "Printing took too long and was stopped. Try again.";
  }
};

const publicFailureMessage = (error: unknown) => {
  if (!(error instanceof Error) || !error.message) return "The print view failed. Try again.";
  if (
    error.message.startsWith("The print") ||
    error.message.startsWith("The document") ||
    error.message.startsWith("The Urdu") ||
    error.message.startsWith("An image") ||
    error.message.startsWith("Creating the preview") ||
    error.message.startsWith("Printing took")
  ) {
    return error.message;
  }
  return "The print view failed. Try again.";
};

const classifyFailure = (error: unknown, signal: AbortSignal): PrintJobResult => {
  const reason = error instanceof PrintAbortedError ? error.reason : signal.reason;
  if (reason === PRINT_ABORT_SUPERSEDED) return { _tag: "Superseded" };
  if (reason === PRINT_ABORT_CANCELLED) return { _tag: "Cancelled" };
  if (error instanceof PrintPhaseTimeoutError) {
    return { _tag: "TimedOut", phase: error.phase, message: error.message };
  }
  if (signal.reason instanceof PrintPhaseTimeoutError) {
    return { _tag: "TimedOut", phase: signal.reason.phase, message: signal.reason.message };
  }
  return { _tag: "Failed", message: publicFailureMessage(error) };
};

const withDeadline = <T>(
  promise: Promise<T>,
  ms: number,
  signal: AbortSignal,
  phase: PrintPhase,
): Promise<T> =>
  new Promise((resolve, reject) => {
    const fail = (error: unknown) => {
      clearTimeout(timer);
      signal.removeEventListener("abort", onAbort);
      reject(error);
    };
    const onAbort = () => fail(new PrintAbortedError(signal.reason));
    const timer = setTimeout(() => fail(new PrintPhaseTimeoutError(phase)), ms);
    if (signal.aborted) {
      onAbort();
      return;
    }
    signal.addEventListener("abort", onAbort, { once: true });
    promise.then(
      (value) => {
        clearTimeout(timer);
        signal.removeEventListener("abort", onAbort);
        resolve(value);
      },
      (error: unknown) => fail(error),
    );
  });

const viewOf = (job: InternalJob): RunningPrintJob => ({
  kind: job.kind,
  requestId: job.requestId,
  origin: job.origin,
  html: job.html,
});

export function createPrintService(
  runner: PrintJobRunner,
  limits: PrintServiceLimits = PRINT_SERVICE_LIMITS,
) {
  let queue: InternalJob[] = [];
  let retainedBytes = 0;
  let running: InternalJob | undefined;
  let runningAbort: AbortController | undefined;
  let draining = false;

  const releaseBytes = (job: InternalJob) => {
    retainedBytes -= job.bytes;
    if (retainedBytes < 0) retainedBytes = 0;
    job.bytes = 0;
    job.html = "";
  };

  const settleQueued = (job: InternalJob, result: PrintJobResult) => {
    releaseBytes(job);
    job.resolve(result);
  };

  const createJob = (request: PrintJobRequest, bytes: number) => {
    let resolve: (result: PrintJobResult) => void = () => undefined;
    const promise = new Promise<PrintJobResult>((done) => {
      resolve = done;
    });
    const job: InternalJob = {
      kind: request.kind,
      owner: request.owner,
      requestId: request.requestId,
      origin: request.origin,
      html: request.html,
      bytes,
      resolve,
    };
    return { job, promise };
  };

  const stopRunning = (reason: string) => {
    if (!running) return;
    releaseBytes(running);
    runningAbort?.abort(reason);
    runner.resetWindow();
  };

  async function execute(job: InternalJob, abort: AbortController): Promise<PrintJobResult> {
    const signal = abort.signal;
    let pdf: Uint8Array | undefined;
    let printed: PrintedJob | undefined;

    for (const phase of phasesFor(job.kind)) {
      if (signal.aborted) {
        runner.resetWindow();
        throw new PrintAbortedError(signal.reason);
      }
      const pending = runner.runPhase(phase, viewOf(job), signal);
      let output: PrintPhaseOutput;
      try {
        output = await withDeadline(pending, limits.deadlines[phase], signal, phase);
      } catch (error) {
        if (!signal.aborted && error instanceof PrintPhaseTimeoutError) abort.abort(error);
        runner.resetWindow();
        void pending.catch(() => undefined);
        throw error;
      }
      if (signal.aborted) {
        runner.resetWindow();
        throw new PrintAbortedError(signal.reason);
      }
      if (phase === "pdf" && output instanceof Uint8Array) pdf = output;
      else if (phase === "print" && isPrintedJob(output)) printed = output;
    }

    if (job.kind === "preview") {
      if (!pdf) {
        runner.resetWindow();
        return { _tag: "Failed", message: "The preview PDF was empty." };
      }
      return { _tag: "Pdf", pdf };
    }
    return {
      _tag: "Printed",
      printed: printed?.printed ?? false,
      failureReason: printed?.failureReason,
    };
  }

  async function runOne(job: InternalJob) {
    const abort = new AbortController();
    running = job;
    runningAbort = abort;
    let outcome: PrintJobResult;
    try {
      outcome = await execute(job, abort);
    } catch (error) {
      outcome = classifyFailure(error, abort.signal);
    } finally {
      releaseBytes(job);
      if (running === job) running = undefined;
      if (runningAbort === abort) runningAbort = undefined;
    }
    if (outcome._tag === "Pdf" || outcome._tag === "Printed") runner.noteIdle();
    job.resolve(outcome);
  }

  function pump() {
    if (draining) return;
    draining = true;
    void drain();
  }

  async function drain() {
    while (queue.length > 0) {
      const job = queue.shift();
      if (!job) break;
      await runOne(job);
    }
    draining = false;
    if (queue.length > 0) pump();
  }

  return {
    submit(request: PrintJobRequest): Promise<PrintJobResult> {
      const bytes = htmlBytes(request.html);
      if (bytes > limits.maxRetainedHtmlBytes) {
        return Promise.resolve({ _tag: "Busy", message: PRINT_TOO_LARGE_MESSAGE });
      }

      if (request.kind === "preview") {
        const index = queue.findIndex(
          (job) => job.kind === "preview" && sameOwner(job, request.owner),
        );
        if (index >= 0) {
          const previous = queue[index];
          if (!previous) return Promise.resolve({ _tag: "Busy", message: PRINT_BUSY_MESSAGE });
          if (retainedBytes - previous.bytes + bytes > limits.maxRetainedHtmlBytes) {
            return Promise.resolve({ _tag: "Busy", message: PRINT_BUSY_MESSAGE });
          }
          const admitted = createJob(request, bytes);
          queue[index] = admitted.job;
          retainedBytes = retainedBytes - previous.bytes + bytes;
          previous.bytes = 0;
          previous.html = "";
          previous.resolve({ _tag: "Superseded" });
          pump();
          return admitted.promise;
        }
        if (running?.kind === "preview" && sameOwner(running, request.owner)) {
          stopRunning(PRINT_ABORT_SUPERSEDED);
        }
      }

      if (
        queue.length >= limits.maxQueuedJobs ||
        retainedBytes + bytes > limits.maxRetainedHtmlBytes
      ) {
        return Promise.resolve({ _tag: "Busy", message: PRINT_BUSY_MESSAGE });
      }

      const admitted = createJob(request, bytes);
      retainedBytes += bytes;
      queue.push(admitted.job);
      pump();
      return admitted.promise;
    },

    cancelPreview(owner: PrintOwner, requestId: string) {
      const index = queue.findIndex(
        (job) => job.kind === "preview" && job.requestId === requestId && sameOwner(job, owner),
      );
      if (index >= 0) {
        const [job] = queue.splice(index, 1);
        if (job) settleQueued(job, { _tag: "Cancelled" });
        return;
      }
      if (
        running?.kind === "preview" &&
        running.requestId === requestId &&
        sameOwner(running, owner)
      ) {
        stopRunning(PRINT_ABORT_CANCELLED);
      }
    },

    releaseWebContents(webContentsId: number) {
      const kept: InternalJob[] = [];
      for (const job of queue) {
        if (job.owner.webContentsId === webContentsId) settleQueued(job, { _tag: "Cancelled" });
        else kept.push(job);
      }
      queue = kept;
      if (running?.owner.webContentsId === webContentsId) stopRunning(PRINT_ABORT_CANCELLED);
    },
  };
}

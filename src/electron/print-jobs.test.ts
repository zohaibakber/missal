import { expect, it } from "vite-plus/test";
import {
  PRINT_BUSY_MESSAGE,
  PRINT_TOO_LARGE_MESSAGE,
  createPrintService,
  type PrintJobRequest,
  type PrintJobResult,
  type PrintJobRunner,
  type PrintPhase,
  type PrintServiceLimits,
} from "#/electron/print-jobs";

const limits = (overrides?: Partial<PrintServiceLimits>): PrintServiceLimits => ({
  maxQueuedJobs: 4,
  maxRetainedHtmlBytes: 1_000,
  deadlines: {
    pageLoad: 1_000,
    layout: 1_000,
    fonts: 1_000,
    images: 1_000,
    pdf: 1_000,
    print: 1_000,
  },
  ...overrides,
});

const request = (
  kind: PrintJobRequest["kind"],
  requestId: string,
  owner: PrintJobRequest["owner"] = { webContentsId: 1, ownerId: "preview" },
  html = "<p>سلام</p>",
): PrintJobRequest => ({ kind, requestId, owner, origin: "missal://renderer", html });

type Gate = { promise: Promise<void>; release: () => void };

const gate = (): Gate => {
  let release: () => void = () => undefined;
  const promise = new Promise<void>((resolve) => {
    release = resolve;
  });
  return { promise, release };
};

function controllableRunner(hangOnPageLoad: ReadonlySet<string> = new Set()) {
  const events: string[] = [];
  let generation = 0;
  const waits = new Map<string, Gate>();

  const waitFor = (requestId: string) => {
    const existing = waits.get(requestId);
    if (existing) return existing;
    const created = gate();
    waits.set(requestId, created);
    return created;
  };

  const runner: PrintJobRunner = {
    noteIdle() {
      events.push("idle");
    },
    resetWindow() {
      generation += 1;
      events.push("reset");
      for (const requestId of hangOnPageLoad) waitFor(requestId).release();
    },
    async runPhase(phase, job, signal) {
      const seen = generation;
      events.push(`${job.requestId}:${phase}`);
      if (signal.aborted) throw new Error("The print view closed.");
      if (phase === "pageLoad" && hangOnPageLoad.has(job.requestId)) {
        await waitFor(job.requestId).promise;
        if (signal.aborted || seen !== generation) throw new Error("The print view closed.");
      }
      if (phase === "pdf") return new Uint8Array([9]);
      if (phase === "print") return { printed: true };
    },
  };

  return {
    runner,
    events,
    release(requestId: string) {
      waitFor(requestId).release();
    },
    generation: () => generation,
  };
}

const phasesOf = (events: readonly string[], requestId: string) =>
  events.filter((event) => event.startsWith(`${requestId}:`));

const outputOrder = (events: readonly string[], phase: PrintPhase) =>
  events.flatMap((event) => {
    const [requestId, eventPhase] = event.split(":");
    return eventPhase === phase && requestId ? [requestId] : [];
  });

it("rejects a job that does not fit without starting it", async () => {
  const fake = controllableRunner(new Set(["hold"]));
  const service = createPrintService(
    fake.runner,
    limits({ maxQueuedJobs: 1, maxRetainedHtmlBytes: 500 }),
  );
  const first = service.submit(request("print", "hold", { webContentsId: 1, ownerId: "a" }, "a"));
  const second = service.submit(
    request("print", "queued", { webContentsId: 1, ownerId: "a" }, "b"),
  );
  const third = service.submit(
    request("print", "overflow", { webContentsId: 1, ownerId: "a" }, "c"),
  );

  await expect(third).resolves.toEqual({ _tag: "Busy", message: PRINT_BUSY_MESSAGE });
  expect(fake.events).not.toContain("overflow:pageLoad");

  fake.release("hold");
  await expect(first).resolves.toMatchObject({ _tag: "Printed", printed: true });
  await expect(second).resolves.toMatchObject({ _tag: "Printed", printed: true });
  expect(outputOrder(fake.events, "print")).toEqual(["hold", "queued"]);
});

it("rejects one document that is larger than the retained HTML budget", async () => {
  const fake = controllableRunner();
  const service = createPrintService(fake.runner, limits({ maxRetainedHtmlBytes: 4 }));
  // "ی" is two UTF-8 bytes, so length 3 is not the admission size.
  const html = "ی".repeat(3);

  await expect(
    service.submit(request("print", "large", { webContentsId: 1, ownerId: "a" }, html)),
  ).resolves.toEqual({ _tag: "Busy", message: PRINT_TOO_LARGE_MESSAGE });
  expect(fake.events).toEqual([]);
});

it("replaces a queued preview from the same owner and keeps accepted prints", async () => {
  const fake = controllableRunner(new Set(["print-1"]));
  const service = createPrintService(fake.runner, limits());
  const owner = { webContentsId: 7, ownerId: "fir" };
  const print1 = service.submit(request("print", "print-1", owner));
  const preview1 = service.submit(request("preview", "preview-1", owner, "<p>old</p>"));
  const print2 = service.submit(request("print", "print-2", owner));
  const preview2 = service.submit(request("preview", "preview-2", owner, "<p>new</p>"));

  await expect(preview1).resolves.toEqual({ _tag: "Superseded" });
  fake.release("print-1");

  await expect(print1).resolves.toMatchObject({ _tag: "Printed", printed: true });
  await expect(preview2).resolves.toMatchObject({ _tag: "Pdf", pdf: new Uint8Array([9]) });
  await expect(print2).resolves.toMatchObject({ _tag: "Printed", printed: true });
  expect(outputOrder(fake.events, "print")).toEqual(["print-1", "print-2"]);
  expect(phasesOf(fake.events, "preview-1")).toEqual([]);
  expect(phasesOf(fake.events, "preview-2").at(-1)).toBe("preview-2:pdf");
});

it("cancels a running preview when the same owner submits a newer one", async () => {
  const fake = controllableRunner(new Set(["old-preview"]));
  const service = createPrintService(fake.runner, limits());
  const owner = { webContentsId: 3, ownerId: "template" };
  const oldPreview = service.submit(request("preview", "old-preview", owner));
  const nextPrint = service.submit(request("print", "kept-print", owner));
  const newPreview = service.submit(request("preview", "new-preview", owner, "<p>newer</p>"));

  await expect(oldPreview).resolves.toEqual({ _tag: "Superseded" });
  expect(fake.events).toContain("reset");
  await expect(nextPrint).resolves.toMatchObject({ _tag: "Printed", printed: true });
  await expect(newPreview).resolves.toMatchObject({ _tag: "Pdf" });
  expect(outputOrder(fake.events, "print")).toEqual(["kept-print"]);
  const resetAt = fake.events.indexOf("reset");
  const nextRunAt = fake.events.indexOf("kept-print:pageLoad");
  expect(resetAt).toBeGreaterThan(-1);
  expect(nextRunAt).toBeGreaterThan(resetAt);
});

it("does not let a preview replace a print from the same owner", async () => {
  const fake = controllableRunner(new Set(["printing"]));
  const service = createPrintService(fake.runner, limits());
  const owner = { webContentsId: 1, ownerId: "fir" };
  const printing = service.submit(request("print", "printing", owner));
  const preview = service.submit(request("preview", "preview", owner));

  expect(fake.events).not.toContain("reset");
  fake.release("printing");
  await expect(printing).resolves.toMatchObject({ _tag: "Printed", printed: true });
  await expect(preview).resolves.toMatchObject({ _tag: "Pdf" });
  expect(outputOrder(fake.events, "pageLoad")).toEqual(["printing", "preview"]);
});

it("resets the window after a timeout and lets the next job succeed", async () => {
  const fake = controllableRunner(new Set(["slow"]));
  const service = createPrintService(
    fake.runner,
    limits({
      deadlines: {
        pageLoad: 30,
        layout: 1_000,
        fonts: 1_000,
        images: 1_000,
        pdf: 1_000,
        print: 1_000,
      },
    }),
  );
  const slow = service.submit(request("preview", "slow"));
  const next = service.submit(request("print", "next", { webContentsId: 1, ownerId: "other" }));

  await expect(slow).resolves.toMatchObject({
    _tag: "TimedOut",
    phase: "pageLoad",
    message: "The print view did not open in time. Try again.",
  });
  await expect(next).resolves.toMatchObject({ _tag: "Printed", printed: true });
  const resetAt = fake.events.indexOf("reset");
  const nextAt = fake.events.indexOf("next:pageLoad");
  expect(resetAt).toBeGreaterThan(-1);
  expect(nextAt).toBeGreaterThan(resetAt);
  expect(fake.generation()).toBeGreaterThan(0);
});

it("resets the window when a job crashes and still runs the following job", async () => {
  let crashed = false;
  const events: string[] = [];
  const runner: PrintJobRunner = {
    noteIdle() {},
    resetWindow() {
      events.push("reset");
    },
    async runPhase(phase, job) {
      events.push(`${job.requestId}:${phase}`);
      if (job.requestId === "bad" && phase === "pageLoad") {
        crashed = true;
        throw new Error("The print view crashed.");
      }
      if (phase === "print") return { printed: true };
    },
  };
  const service = createPrintService(runner, limits());
  const failed = service.submit(request("print", "bad"));
  const next = service.submit(request("print", "good"));

  await expect(failed).resolves.toEqual({
    _tag: "Failed",
    message: "The print view crashed.",
  });
  await expect(next).resolves.toMatchObject({ _tag: "Printed", printed: true });
  expect(crashed).toBe(true);
  expect(events.indexOf("reset")).toBeLessThan(events.indexOf("good:pageLoad"));
});

it("cancels the closed window's queued and running jobs and keeps the other window's print", async () => {
  const fake = controllableRunner(new Set(["running-print"]));
  const service = createPrintService(fake.runner, limits());
  const closing = { webContentsId: 4, ownerId: "fir" };
  const staying = { webContentsId: 8, ownerId: "fir" };
  const runningPrint = service.submit(request("print", "running-print", closing));
  const queuedPreview = service.submit(request("preview", "queued-preview", closing));
  const otherPrint = service.submit(request("print", "other-print", staying));

  service.releaseWebContents(closing.webContentsId);

  await expect(runningPrint).resolves.toEqual({ _tag: "Cancelled" });
  await expect(queuedPreview).resolves.toEqual({ _tag: "Cancelled" });
  await expect(otherPrint).resolves.toMatchObject({ _tag: "Printed", printed: true });
  expect(phasesOf(fake.events, "queued-preview")).toEqual([]);
  expect(outputOrder(fake.events, "print")).toEqual(["other-print"]);
  expect(fake.events).toContain("reset");
});

it("cancels one preview without dropping an accepted print", async () => {
  const fake = controllableRunner(new Set(["printing"]));
  const service = createPrintService(fake.runner, limits());
  const owner = { webContentsId: 2, ownerId: "fir" };
  const printing = service.submit(request("print", "printing", owner));
  const preview = service.submit(request("preview", "preview", owner));

  service.cancelPreview(owner, "preview");
  fake.release("printing");

  await expect(preview).resolves.toEqual({ _tag: "Cancelled" });
  await expect(printing).resolves.toMatchObject({ _tag: "Printed", printed: true });
  expect(phasesOf(fake.events, "preview")).toEqual([]);
});

it("counts retained HTML in UTF-8 bytes while a job is running", async () => {
  const fake = controllableRunner(new Set(["first"]));
  const service = createPrintService(fake.runner, limits({ maxRetainedHtmlBytes: 20 }));
  const html = "ی".repeat(10);
  expect(new TextEncoder().encode(html).byteLength).toBe(20);

  const first = service.submit(request("print", "first", { webContentsId: 1, ownerId: "a" }, html));
  const second = service.submit(
    request("print", "second", { webContentsId: 1, ownerId: "a" }, html),
  );

  await expect(second).resolves.toEqual({ _tag: "Busy", message: PRINT_BUSY_MESSAGE });
  fake.release("first");
  const result: PrintJobResult = await first;
  expect(result._tag).toBe("Printed");
});

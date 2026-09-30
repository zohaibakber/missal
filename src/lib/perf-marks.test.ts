import { afterEach, expect, it } from "@effect/vitest";
import { perfMark, perfNow, snapshotPerfMarks } from "#/lib/perf-marks";

const previous = process.env.MISSAL_PERF;
afterEach(() => {
  if (previous === undefined) delete process.env.MISSAL_PERF;
  else process.env.MISSAL_PERF = previous;
});

it("records nothing unless performance marks are enabled", () => {
  delete process.env.MISSAL_PERF;
  const before = snapshotPerfMarks().marks.length;
  expect(perfNow()).toBe(0);
  perfMark("storage.decode", { startedAt: 1, chars: 10 });
  expect(snapshotPerfMarks().marks.length).toBe(before);
});

it("records the operation, its duration, and its size", () => {
  process.env.MISSAL_PERF = "1";
  const startedAt = perfNow();
  perfMark("storage.execute", { startedAt, chars: 8, errorCategory: "StorageError" });
  expect(snapshotPerfMarks().marks.at(-1)).toMatchObject({
    name: "storage.execute",
    chars: 8,
    errorCategory: "StorageError",
    durationMs: expect.any(Number),
  });
});

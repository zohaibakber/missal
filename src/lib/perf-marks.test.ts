import { expect, it } from "@effect/vitest";
import { perfMark, perfNow, snapshotPerfMarks } from "#/lib/perf-marks";

it("records nothing unless performance marks are enabled", () => {
  const previous = process.env.MISSAL_PERF;
  delete process.env.MISSAL_PERF;
  const before = snapshotPerfMarks().marks.length;
  expect(perfNow()).toBe(0);
  perfMark("storage.decode", { bytesIn: "متن دستاویز", startedAt: 1 });
  expect(snapshotPerfMarks().marks.length).toBe(before);
  if (previous === undefined) delete process.env.MISSAL_PERF;
  else process.env.MISSAL_PERF = previous;
});

it("records operation, duration, and byte size without document text", () => {
  const previous = process.env.MISSAL_PERF;
  process.env.MISSAL_PERF = "1";
  try {
    const startedAt = performance.now();
    perfMark("storage.execute", { startedAt, bytesIn: "سلام", errorCategory: "StorageError" });
    const mark = snapshotPerfMarks().marks.at(-1);
    expect(mark?.name).toBe("storage.execute");
    expect(mark?.errorCategory).toBe("StorageError");
    expect(mark?.bytesIn).toBe(new TextEncoder().encode("سلام").byteLength);
    expect(mark && "text" in mark).toBe(false);
    expect(JSON.stringify(mark)).not.toContain("سلام");
  } finally {
    if (previous === undefined) delete process.env.MISSAL_PERF;
    else process.env.MISSAL_PERF = previous;
  }
});

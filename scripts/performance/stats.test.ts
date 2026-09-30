import { expect, it } from "@effect/vitest";
import { compareBench, compareMetric, median, percentile } from "./stats.ts";

it("computes median and nearest-rank p95", () => {
  expect(median([3, 1, 2])).toBe(2);
  expect(median([1, 2, 3, 4])).toBe(2.5);
  expect(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 95)).toBe(10);
  expect(percentile([10, 20], 95)).toBe(20);
});

it("flags a regression past measured variation and the 10% threshold", () => {
  const result = compareMetric(
    "documentSave10.samplesMs",
    [100, 100, 100, 100],
    [130, 130, 130, 130],
  );
  expect(result.beyondVariation).toBe(true);
  expect(result.beyondThreshold).toBe(true);
  expect(result.regression).toBe(true);
});

it("keeps a change that stays inside measured variation", () => {
  const result = compareMetric("firList.samplesMs", [100, 100, 110, 150], [104, 104, 108, 140]);
  expect(result.beyondVariation).toBe(false);
  expect(result.flagged).toBe(false);
  expect(result.regression).toBe(false);
});

it("does not treat a sub-threshold change as a regression when spread is zero", () => {
  const result = compareMetric("templateList.samplesMs", [100, 100, 100], [105, 105, 105]);
  expect(result.beyondVariation).toBe(true);
  expect(result.beyondThreshold).toBe(false);
  expect(result.regression).toBe(false);
});

it("records a large improvement without calling it a regression", () => {
  const result = compareMetric("documentGet.samplesMs", [200, 200, 200], [100, 100, 100]);
  expect(result.flagged).toBe(true);
  expect(result.regression).toBe(false);
  expect(result.medianDelta).toBeLessThan(0);
});

it("compares scenario medians and fails closed on a slower save", () => {
  const scenario = {
    samplesMs: [10, 10, 10],
    heapDeltaBytes: [0, 0, 0],
    rssDeltaBytes: [0, 0, 0],
  };
  const report = compareBench(
    { fixture: { sha256: "abc" }, scenarios: { documentSave10: scenario } },
    {
      fixture: { sha256: "abc" },
      scenarios: {
        documentSave10: { ...scenario, samplesMs: [20, 20, 20] },
      },
    },
  );
  expect(report.fixtureMismatch).toBe(false);
  expect(report.regressions).toContain("documentSave10.samplesMs");
});

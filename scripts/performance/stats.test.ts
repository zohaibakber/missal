import { expect, it } from "@effect/vitest";
import { compareBench, compareMetric, median, percentile } from "./stats.ts";

it("computes median and nearest-rank p95", () => {
  expect(median([3, 1, 2])).toBe(2);
  expect(median([1, 2, 3, 4])).toBe(2.5);
  expect(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 95)).toBe(10);
  expect(percentile([10, 20], 95)).toBe(20);
});

it("flags a regression past measured variation and the 10% threshold", () => {
  expect(compareMetric("save", [100, 100, 100, 100], [130, 130, 130, 130])).toMatchObject({
    flagged: true,
    regression: true,
  });
});

it("ignores a change inside measured variation", () => {
  expect(compareMetric("list", [100, 100, 110, 150], [104, 104, 108, 140]).flagged).toBe(false);
});

it("ignores a change under the threshold even when spread is zero", () => {
  expect(compareMetric("templates", [100, 100, 100], [105, 105, 105]).flagged).toBe(false);
});

it("flags a large improvement without calling it a regression", () => {
  expect(compareMetric("get", [200, 200, 200], [100, 100, 100])).toMatchObject({
    flagged: true,
    regression: false,
  });
});

it("reports a slower save scenario and a changed fixture", () => {
  const scenario = { samplesMs: [10, 10, 10], heapDeltaBytes: [0, 0, 0], rssDeltaBytes: [0, 0, 0] };
  const report = compareBench(
    { fixture: { sha256: "abc" }, scenarios: { documentSave10: scenario } },
    {
      fixture: { sha256: "def" },
      scenarios: { documentSave10: { ...scenario, samplesMs: [20, 20, 20] } },
    },
  );
  expect(report.fixtureMismatch).toBe(true);
  expect(report.regressions).toEqual(["documentSave10.samplesMs"]);
});

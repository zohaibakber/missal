const SCREENING_THRESHOLD = 0.1;

const sorted = (samples: readonly number[]) => {
  if (samples.length === 0) throw new Error("No samples");
  return samples.toSorted((left, right) => left - right);
};

export function median(samples: readonly number[]) {
  const values = sorted(samples);
  const mid = Math.floor(values.length / 2);
  const upper = values[mid] ?? 0;
  return values.length % 2 === 0 ? ((values[mid - 1] ?? 0) + upper) / 2 : upper;
}

/** Nearest-rank percentile. */
export function percentile(samples: readonly number[], rank: number) {
  const values = sorted(samples);
  const index = Math.min(values.length, Math.max(1, Math.ceil((rank / 100) * values.length)));
  return values[index - 1] ?? 0;
}

type MetricComparison = {
  readonly name: string;
  readonly baselineMedian: number;
  readonly baselineP95: number;
  readonly candidateMedian: number;
  readonly candidateP95: number;
  readonly flagged: boolean;
  readonly regression: boolean;
};

const pastThreshold = (baseline: number, candidate: number) =>
  baseline === 0
    ? candidate !== 0
    : Math.abs(candidate - baseline) / Math.abs(baseline) > SCREENING_THRESHOLD;

// A change counts only when it exceeds both the samples' own p95-median spread and the
// screening threshold.
function compareMetric(
  name: string,
  baseline: readonly number[],
  candidate: readonly number[],
): MetricComparison {
  const baselineMedian = median(baseline);
  const candidateMedian = median(candidate);
  const baselineP95 = percentile(baseline, 95);
  const candidateP95 = percentile(candidate, 95);
  const medianDelta = candidateMedian - baselineMedian;
  const p95Delta = candidateP95 - baselineP95;
  const variation = Math.max(baselineP95 - baselineMedian, candidateP95 - candidateMedian, 0);
  const flagged =
    (Math.abs(medianDelta) > variation || Math.abs(p95Delta) > variation) &&
    (pastThreshold(baselineMedian, candidateMedian) || pastThreshold(baselineP95, candidateP95));
  return {
    name,
    baselineMedian,
    baselineP95,
    candidateMedian,
    candidateP95,
    flagged,
    regression: flagged && (medianDelta > 0 || p95Delta > 0),
  };
}

type ScenarioSamples = {
  readonly samplesMs: readonly number[];
  readonly heapDeltaBytes: readonly number[];
  readonly rssDeltaBytes: readonly number[];
};

export type BenchReport = {
  readonly fixture?: { readonly sha256?: string };
  readonly scenarios: Readonly<Record<string, ScenarioSamples>>;
};

const SAMPLE_KEYS = ["samplesMs", "heapDeltaBytes", "rssDeltaBytes"] as const;

export function compareBench(baseline: BenchReport, candidate: BenchReport) {
  const names = [
    ...new Set([...Object.keys(baseline.scenarios), ...Object.keys(candidate.scenarios)]),
  ].sort();
  const metrics = names.flatMap((name) => {
    const left = baseline.scenarios[name];
    const right = candidate.scenarios[name];
    if (!left || !right) throw new Error(`Scenario ${name} is missing from one result`);
    return SAMPLE_KEYS.map((key) => compareMetric(`${name}.${key}`, left[key], right[key]));
  });
  return {
    metrics,
    regressions: metrics.filter((metric) => metric.regression).map((metric) => metric.name),
    fixtureMismatch: baseline.fixture?.sha256 !== candidate.fixture?.sha256,
  };
}

export function formatComparison(report: ReturnType<typeof compareBench>) {
  const lines = ["metric\tbaselineMedian\tcandidateMedian\tbaselineP95\tcandidateP95\tflag"];
  for (const metric of report.metrics) {
    const flag = metric.regression ? "regression" : metric.flagged ? "flagged" : "";
    lines.push(
      [
        metric.name,
        metric.baselineMedian,
        metric.candidateMedian,
        metric.baselineP95,
        metric.candidateP95,
        flag,
      ].join("\t"),
    );
  }
  if (report.fixtureMismatch) lines.push("fixture sha256 differs; this is not the same dataset");
  if (report.regressions.length === 0) {
    lines.push("no regressions past measured variation and the 10% screening threshold");
  }
  return lines.join("\n");
}

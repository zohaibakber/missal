/** Nearest-rank percentile. Rank is ceil(p/100 * n), clamped to the sorted sample. */
export const SCREENING_THRESHOLD = 0.1;

export function median(samples: readonly number[]) {
  if (samples.length === 0) throw new Error("No samples");
  const sorted = [...samples].sort((left, right) => left - right);
  const mid = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 0) return ((sorted[mid - 1] as number) + (sorted[mid] as number)) / 2;
  return sorted[mid] as number;
}

export function percentile(samples: readonly number[], rank: number) {
  if (samples.length === 0) throw new Error("No samples");
  if (rank <= 0 || rank > 100) throw new Error(`Percentile ${rank} is outside 1..100`);
  const sorted = [...samples].sort((left, right) => left - right);
  const index = Math.min(sorted.length, Math.max(1, Math.ceil((rank / 100) * sorted.length))) - 1;
  return sorted[index] as number;
}

export type MetricComparison = {
  readonly name: string;
  readonly baselineMedian: number;
  readonly baselineP95: number;
  readonly candidateMedian: number;
  readonly candidateP95: number;
  readonly medianDelta: number;
  readonly p95Delta: number;
  /** Null when the baseline is 0 and the candidate is not. */
  readonly medianRatio: number | null;
  readonly p95Ratio: number | null;
  /** Wider of the two p95-median spreads. A delta inside this is measurement noise. */
  readonly variation: number;
  readonly beyondVariation: boolean;
  readonly beyondThreshold: boolean;
  readonly flagged: boolean;
  readonly regression: boolean;
};

function ratio(baseline: number, candidate: number) {
  if (baseline === 0) return candidate === 0 ? 0 : null;
  return (candidate - baseline) / Math.abs(baseline);
}

function beyondScreeningThreshold(change: number | null, absoluteDelta: number) {
  if (change === null) return absoluteDelta !== 0;
  return Math.abs(change) > SCREENING_THRESHOLD;
}

export function compareMetric(
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
  const medianRatio = ratio(baselineMedian, candidateMedian);
  const p95Ratio = ratio(baselineP95, candidateP95);
  const variation = Math.max(baselineP95 - baselineMedian, candidateP95 - candidateMedian, 0);
  const beyondVariation = Math.abs(medianDelta) > variation || Math.abs(p95Delta) > variation;
  const beyondThreshold =
    beyondScreeningThreshold(medianRatio, medianDelta) ||
    beyondScreeningThreshold(p95Ratio, p95Delta);
  const flagged = beyondVariation && beyondThreshold;
  const regression = flagged && (medianDelta > 0 || p95Delta > 0);
  return {
    name,
    baselineMedian,
    baselineP95,
    candidateMedian,
    candidateP95,
    medianDelta,
    p95Delta,
    medianRatio,
    p95Ratio,
    variation,
    beyondVariation,
    beyondThreshold,
    flagged,
    regression,
  };
}

export type ScenarioSamples = {
  readonly samplesMs: readonly number[];
  readonly heapDeltaBytes: readonly number[];
  readonly rssDeltaBytes: readonly number[];
};

export type BenchReport = {
  readonly fixture?: { readonly sha256?: string };
  readonly scenarios: Readonly<Record<string, ScenarioSamples>>;
};

export type CompareReport = {
  readonly metrics: readonly MetricComparison[];
  readonly regressions: readonly string[];
  readonly fixtureMismatch: boolean;
};

const SAMPLE_KEYS = ["samplesMs", "heapDeltaBytes", "rssDeltaBytes"] as const;

export function compareBench(baseline: BenchReport, candidate: BenchReport): CompareReport {
  const fixtureMismatch =
    baseline.fixture?.sha256 !== undefined &&
    candidate.fixture?.sha256 !== undefined &&
    baseline.fixture.sha256 !== candidate.fixture.sha256;
  const names = [
    ...new Set([...Object.keys(baseline.scenarios), ...Object.keys(candidate.scenarios)]),
  ].sort();
  const metrics: MetricComparison[] = [];
  for (const name of names) {
    const left = baseline.scenarios[name];
    const right = candidate.scenarios[name];
    if (!left || !right) {
      throw new Error(`Scenario ${name} is missing from one result`);
    }
    for (const key of SAMPLE_KEYS) {
      metrics.push(compareMetric(`${name}.${key}`, left[key], right[key]));
    }
  }
  return {
    metrics,
    regressions: metrics.filter((metric) => metric.regression).map((metric) => metric.name),
    fixtureMismatch,
  };
}

export function formatComparison(report: CompareReport) {
  const lines = [
    "metric\tbaselineMedian\tcandidateMedian\tmedianDelta\tbaselineP95\tcandidateP95\tp95Delta\tflag",
  ];
  for (const metric of report.metrics) {
    let flag = "";
    if (metric.regression) flag = "regression";
    else if (metric.flagged) flag = "flagged";
    lines.push(
      [
        metric.name,
        metric.baselineMedian,
        metric.candidateMedian,
        metric.medianDelta,
        metric.baselineP95,
        metric.candidateP95,
        metric.p95Delta,
        flag,
      ].join("\t"),
    );
  }
  if (report.fixtureMismatch) {
    lines.push("fixture sha256 differs; this is not the same dataset");
  }
  if (report.regressions.length === 0) {
    lines.push("no regressions past measured variation and the 10% screening threshold");
  }
  return lines.join("\n");
}

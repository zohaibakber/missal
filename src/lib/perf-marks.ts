import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

const PERF_MARK_LIMIT = 2048;

// Names, sizes, and error tags only: marks must never carry document text.
type PerfMark = {
  readonly name: string;
  readonly id: number;
  readonly atMs: number;
  readonly durationMs?: number;
  readonly chars?: number;
  readonly errorCategory?: string;
};

const marks: PerfMark[] = [];
let sequence = 0;

const perfEnabled = () => process.env.MISSAL_PERF === "1";

export function perfNow() {
  return perfEnabled() ? performance.now() : 0;
}

export function perfMark(
  name: string,
  details: {
    readonly startedAt?: number;
    readonly chars?: number;
    readonly errorCategory?: string | undefined;
  } = {},
) {
  if (!perfEnabled()) return;
  const atMs = performance.now();
  sequence += 1;
  marks.push({
    name,
    id: sequence,
    atMs,
    ...(details.startedAt === undefined ? {} : { durationMs: atMs - details.startedAt }),
    ...(details.chars === undefined ? {} : { chars: details.chars }),
    ...(details.errorCategory === undefined ? {} : { errorCategory: details.errorCategory }),
  });
  if (marks.length > PERF_MARK_LIMIT) marks.shift();
}

export function dumpPerfMarks() {
  if (!perfEnabled()) return;
  const file = process.env.MISSAL_PERF_LOG ?? path.join(process.cwd(), ".perf", "perf-marks.json");
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(
    file,
    `${JSON.stringify({ maxEntries: PERF_MARK_LIMIT, dropped: sequence - marks.length, marks }, null, 2)}\n`,
  );
}

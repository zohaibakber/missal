import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

const PERF_MARK_LIMIT = 2048;

export type PerfMark = {
  readonly name: string;
  readonly id: number;
  readonly durationMs: number;
  readonly bytesIn?: number;
  readonly bytesOut?: number;
  readonly errorCategory?: string;
};

const buffer: Array<PerfMark | undefined> = Array.from({ length: PERF_MARK_LIMIT });
let next = 0;
let count = 0;
let sequence = 0;
let dumped = false;

const globalPerf = globalThis as { __MISSAL_PERF__?: boolean };

function perfEnabled() {
  if (typeof process !== "undefined" && process.env.MISSAL_PERF === "1") return true;
  return globalPerf.__MISSAL_PERF__ === true;
}

function byteLength(value: number | string | undefined) {
  if (typeof value === "number") return value;
  if (typeof value === "string") return new TextEncoder().encode(value).byteLength;
  return undefined;
}

export function perfNow() {
  if (!perfEnabled()) return 0;
  return performance.now();
}

export function perfMark(
  name: string,
  details?: {
    readonly startedAt?: number;
    readonly bytesIn?: number | string;
    readonly bytesOut?: number | string;
    readonly errorCategory?: string;
  },
) {
  if (!perfEnabled()) return;
  sequence += 1;
  const bytesIn = byteLength(details?.bytesIn);
  const bytesOut = byteLength(details?.bytesOut);
  const mark: PerfMark = {
    name,
    id: sequence,
    durationMs:
      details?.startedAt === undefined ? performance.now() : performance.now() - details.startedAt,
    ...(bytesIn === undefined ? {} : { bytesIn }),
    ...(bytesOut === undefined ? {} : { bytesOut }),
    ...(details?.errorCategory === undefined ? {} : { errorCategory: details.errorCategory }),
  };
  buffer[next] = mark;
  next = (next + 1) % PERF_MARK_LIMIT;
  if (count < PERF_MARK_LIMIT) count += 1;
}

export function perfTimed<T>(name: string, run: () => T, errorCategory?: string) {
  if (!perfEnabled()) return run();
  const startedAt = performance.now();
  try {
    const value = run();
    perfMark(name, {
      startedAt,
      bytesOut: typeof value === "string" ? value : undefined,
      errorCategory,
    });
    return value;
  } catch (error) {
    perfMark(name, { startedAt, errorCategory: errorCategory ?? categoryOf(error) });
    throw error;
  }
}

export function snapshotPerfMarks() {
  const marks: PerfMark[] = [];
  const start = count < PERF_MARK_LIMIT ? 0 : next;
  for (let index = 0; index < count; index += 1) {
    const mark = buffer[(start + index) % PERF_MARK_LIMIT];
    if (mark) marks.push(mark);
  }
  return {
    maxEntries: PERF_MARK_LIMIT,
    dropped: Math.max(0, sequence - marks.length),
    marks,
  };
}

export function dumpPerfMarksOnQuit() {
  if (!perfEnabled() || dumped) return;
  dumped = true;
  const file = process.env.MISSAL_PERF_LOG ?? path.join(process.cwd(), ".perf", "perf-marks.json");
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, `${JSON.stringify(snapshotPerfMarks(), null, 2)}\n`);
}

function categoryOf(error: unknown) {
  if (typeof error === "object" && error !== null && "_tag" in error) {
    return String((error as { _tag: unknown })._tag);
  }
  return "thrown";
}

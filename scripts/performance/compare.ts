import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { compareBench, formatComparison, type BenchReport } from "./stats.ts";

const { positionals } = parseArgs({ allowPositionals: true });
const [baselinePath, candidatePath] = positionals;
if (!baselinePath || !candidatePath) {
  console.error("Usage: vp run perf:compare -- <baseline.json> <candidate.json>");
  process.exit(2);
}

const read = (file: string) => JSON.parse(readFileSync(file, "utf8")) as BenchReport;
const report = compareBench(read(baselinePath), read(candidatePath));
console.log(formatComparison(report));
if (report.fixtureMismatch) process.exit(2);
process.exit(report.regressions.length > 0 ? 1 : 0);

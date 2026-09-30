import { readFileSync } from "node:fs";
import { readArgs } from "./cli.ts";
import { compareBench, formatComparison, type BenchReport } from "./stats.ts";

const { options, positionals } = readArgs(process.argv.slice(2));
const baselinePath = positionals[0];
const candidatePath = positionals[1];
if (options.help === true || !baselinePath || !candidatePath) {
  console.error("Usage: vp run perf:compare -- <baseline.json> <candidate.json>");
  process.exit(options.help === true ? 0 : 2);
}

const baseline = JSON.parse(readFileSync(baselinePath, "utf8")) as BenchReport;
const candidate = JSON.parse(readFileSync(candidatePath, "utf8")) as BenchReport;
const report = compareBench(baseline, candidate);
console.log(formatComparison(report));
if (report.fixtureMismatch) process.exit(2);
process.exit(report.regressions.length > 0 ? 1 : 0);

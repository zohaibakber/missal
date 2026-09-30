import { parseArgs } from "node:util";
import { generateFixture, parseFixtureName, repoRootFromHere } from "./fixture.ts";

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: { seed: { type: "string", default: "1" }, out: { type: "string" } },
});
const seed = Number(values.seed);
if (positionals.length !== 1 || !Number.isInteger(seed)) {
  console.error(
    "Usage: vp run perf:fixtures -- <small|normal|stress> [--seed 1] [--out .perf/small]",
  );
  process.exit(2);
}

const metadata = await generateFixture({
  fixture: parseFixtureName(positionals[0] ?? ""),
  seed,
  outDir: values.out,
  repoRoot: repoRootFromHere(),
});
console.log(metadata.databasePath);
console.log(`sha256 ${metadata.sha256}`);
console.log(
  `${metadata.counts.firs} FIRs, ${metadata.counts.bundledTemplates} bundled templates, ${metadata.counts.userTemplates} user templates, ${metadata.byteSizes.database} database bytes`,
);

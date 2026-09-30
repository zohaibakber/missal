import { generateFixture, parseFixtureName, repoRootFromHere } from "./fixture.ts";
import { readArgs } from "./cli.ts";

const { options, positionals } = readArgs(process.argv.slice(2));
const fixtureName = String(options.fixture ?? positionals[0] ?? "");
if (!fixtureName || options.help === true) {
  console.error(
    "Usage: vp run perf:fixtures -- <small|normal|stress> [--seed 1] [--out .perf/small]",
  );
  process.exit(options.help === true ? 0 : 2);
}

const seed = Number(options.seed ?? 1);
if (!Number.isInteger(seed)) {
  console.error("--seed must be an integer");
  process.exit(2);
}

const metadata = await generateFixture({
  fixture: parseFixtureName(fixtureName),
  seed,
  outDir: typeof options.out === "string" ? options.out : undefined,
  repoRoot: repoRootFromHere(),
});
console.log(`${metadata.databasePath}`);
console.log(`sha256 ${metadata.sha256}`);
console.log(
  `${metadata.counts.firs} FIRs, ${metadata.counts.bundledTemplates} bundled templates, ${metadata.counts.userTemplates} user templates, ${metadata.byteSizes.database} database bytes`,
);

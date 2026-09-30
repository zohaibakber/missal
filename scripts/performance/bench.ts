import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Effect } from "effect";
import { makeStorageRuntime } from "#/electron/storage-runtime";
import { FirDocumentSaveInput } from "#/lib/fir-document";
import { FirDocumentId } from "#/lib/ids";
import { FirDocumentRepository, FirRepository, TemplateRepository } from "#/repositories/index";
import { readArgs } from "./cli";
import { buildDocument } from "./documents";
import { collectEnvironment } from "./environment";
import { parseFixtureName, repoRootFromHere, type FixtureMetadata } from "./fixture";
import { median, percentile } from "./stats";

type ScenarioResult = {
  readonly samplesMs: number[];
  readonly heapDeltaBytes: number[];
  readonly rssDeltaBytes: number[];
  readonly medianMs: number;
  readonly p95Ms: number;
};

const { options, positionals } = readArgs(process.argv.slice(2));
const fixtureName = String(options.fixture ?? positionals[0] ?? "small");
if (options.help === true) {
  console.error(
    "Usage: vp run perf:bench -- [small|normal|stress] [--samples 10] [--launch-samples 10] [--warmup 2] [--out .perf/small/bench.json]",
  );
  process.exit(0);
}

const fixture = parseFixtureName(fixtureName);
const repoRoot = repoRootFromHere();
const warmup = numberOption(options.warmup, 2);
const samples = numberOption(options.samples, 10);
const launchSamples = numberOption(options["launch-samples"], 10);
const directory = path.join(repoRoot, ".perf", fixture);
const metadata = JSON.parse(
  readFileSync(path.join(directory, "fixture.json"), "utf8"),
) as FixtureMetadata;
const migrationsFolder = path.join(repoRoot, "drizzle");
const scenarios: Record<string, ScenarioResult> = {};

scenarios.openMigrate = await measureSamples(warmup, launchSamples, async () => {
  const copyDir = mkdtempSync(path.join(tmpdir(), "missal-perf-open-"));
  const databasePath = path.join(copyDir, "missal.sqlite");
  copyFileSync(metadata.databasePath, databasePath);
  const runtime = makeStorageRuntime({ databasePath, migrationsFolder });
  const started = performance.now();
  const before = process.memoryUsage();
  try {
    await runtime.runPromise(Effect.flatMap(FirRepository, () => Effect.void));
    return elapsed(started, before);
  } finally {
    await runtime.dispose();
    rmSync(copyDir, { recursive: true, force: true });
  }
});

const workDir = mkdtempSync(path.join(tmpdir(), "missal-perf-work-"));
const workDatabase = path.join(workDir, "missal.sqlite");
copyFileSync(metadata.databasePath, workDatabase);
const runtime = makeStorageRuntime({ databasePath: workDatabase, migrationsFolder });
try {
  await runtime.runPromise(Effect.flatMap(FirRepository, () => Effect.void));
  const documentId = FirDocumentId.make(metadata.probes.documentId);
  const documentIds = metadata.probes.documentIds.map((id) => FirDocumentId.make(id));
  const tenPage = buildDocument({
    repoRoot,
    pages: 10,
    pageOffset: 0,
    extraFieldsPerPage: 2,
    label: "representative",
  }).envelope;
  let revision = (
    await runtime.runPromise(Effect.flatMap(FirDocumentRepository, (repo) => repo.get(documentId)))
  ).revision;

  scenarios.firList = await measureSamples(warmup, samples, () =>
    time(() => runtime.runPromise(Effect.flatMap(FirRepository, (repo) => repo.list))),
  );
  scenarios.templateList = await measureSamples(warmup, samples, () =>
    time(() => runtime.runPromise(Effect.flatMap(TemplateRepository, (repo) => repo.list))),
  );
  scenarios.templateSearch = await measureSamples(warmup, samples, () =>
    time(() =>
      runtime.runPromise(
        Effect.flatMap(TemplateRepository, (repo) => repo.search(metadata.probes.templateSearch)),
      ),
    ),
  );
  scenarios.documentGet = await measureSamples(warmup, samples, () =>
    time(() =>
      runtime.runPromise(Effect.flatMap(FirDocumentRepository, (repo) => repo.get(documentId))),
    ),
  );
  scenarios.documentGetMany = await measureSamples(warmup, samples, () =>
    time(() =>
      runtime.runPromise(
        Effect.flatMap(FirDocumentRepository, (repo) => repo.getMany(documentIds)),
      ),
    ),
  );
  scenarios.documentSave10 = await measureSamples(warmup, samples, () =>
    time(async () => {
      const ack = await runtime.runPromise(
        Effect.flatMap(FirDocumentRepository, (repo) =>
          repo.save(
            new FirDocumentSaveInput({
              id: documentId,
              expectedRevision: revision,
              document: tenPage,
            }),
          ),
        ),
      );
      revision = ack.revision;
    }),
  );
} finally {
  await runtime.dispose();
  rmSync(workDir, { recursive: true, force: true });
}

const report = {
  fixture: {
    name: metadata.fixture,
    seed: metadata.seed,
    sha256: metadata.sha256,
    databasePath: metadata.databasePath,
  },
  warmup,
  samples,
  launchSamples,
  environment: collectEnvironment(repoRoot),
  scenarios,
};
const output =
  typeof options.out === "string" ? path.resolve(options.out) : path.join(directory, "bench.json");
writeFileSync(output, `${JSON.stringify(report, null, 2)}\n`);
console.log(output);
for (const [name, scenario] of Object.entries(scenarios)) {
  console.log(
    `${name} median ${scenario.medianMs.toFixed(2)} ms  p95 ${scenario.p95Ms.toFixed(2)} ms`,
  );
}

function numberOption(value: string | boolean | undefined, fallback: number) {
  if (value === undefined) return fallback;
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0)
    throw new Error(`Expected a non-negative integer, got ${value}`);
  return parsed;
}

type Timed = {
  readonly ms: number;
  readonly heapDeltaBytes: number;
  readonly rssDeltaBytes: number;
};

function elapsed(started: number, before: NodeJS.MemoryUsage): Timed {
  const after = process.memoryUsage();
  return {
    ms: performance.now() - started,
    heapDeltaBytes: after.heapUsed - before.heapUsed,
    rssDeltaBytes: after.rss - before.rss,
  };
}

async function time(run: () => Promise<unknown>): Promise<Timed> {
  const before = process.memoryUsage();
  const started = performance.now();
  await run();
  return elapsed(started, before);
}

async function measureSamples(
  warmupCount: number,
  sampleCount: number,
  run: () => Promise<Timed>,
): Promise<ScenarioResult> {
  for (let index = 0; index < warmupCount; index += 1) await run();
  const samplesMs: number[] = [];
  const heapDeltaBytes: number[] = [];
  const rssDeltaBytes: number[] = [];
  for (let index = 0; index < sampleCount; index += 1) {
    const timed = await run();
    samplesMs.push(timed.ms);
    heapDeltaBytes.push(timed.heapDeltaBytes);
    rssDeltaBytes.push(timed.rssDeltaBytes);
  }
  return {
    samplesMs,
    heapDeltaBytes,
    rssDeltaBytes,
    medianMs: median(samplesMs),
    p95Ms: percentile(samplesMs, 95),
  };
}

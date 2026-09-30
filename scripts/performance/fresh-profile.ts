import "effect/schema/SchemaJITCompiler/enable";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { gzipSync } from "node:zlib";
import { Effect, Schedule } from "effect";
import { makeStorageRuntime } from "#/electron/storage-runtime";
import { templatePackBodyPath } from "#/lib/bundled-templates";
import { TemplateRepository } from "#/repositories/index";
import { buildDocument, loadSourceCorpus } from "./documents";
import { collectEnvironment } from "./environment";
import { repoRootFromHere } from "./fixture";
import { mulberry32, pickInt } from "./prng";
import { median, percentile } from "./stats";

const repoRoot = repoRootFromHere();
const migrationsFolder = path.join(repoRoot, "drizzle");
const warmup = 2;
const samples = 10;

const shipped = await measurePack("shipped", path.join(repoRoot, "bundled-templates"));
const syntheticDirectory = path.join(repoRoot, ".perf", "fresh-synthetic");
rmSync(syntheticDirectory, { recursive: true, force: true });
writeSyntheticPack(syntheticDirectory, 100);
buildSeed(syntheticDirectory);
const synthetic = await measurePack("synthetic-100", syntheticDirectory);

const report = {
  warmup,
  samples,
  environment: collectEnvironment(repoRoot),
  packs: { shipped, synthetic },
};
const output = path.join(repoRoot, ".perf", "fresh-profile.json");
mkdirSync(path.dirname(output), { recursive: true });
writeFileSync(output, `${JSON.stringify(report, null, 2)}\n`);
console.log(output);
for (const pack of [shipped, synthetic]) {
  console.log(
    `${pack.name} templates ${pack.templates} seed ${pack.seedBytes} gz / ${pack.uncompressedBytes} db ${pack.databaseBytes}`,
  );
  console.log(
    `  seed median ${pack.seed.medianMs.toFixed(2)} ms  p95 ${pack.seed.p95Ms.toFixed(2)} ms`,
  );
  console.log(
    `  installer median ${pack.installer.medianMs.toFixed(2)} ms  p95 ${pack.installer.p95Ms.toFixed(2)} ms`,
  );
}

function writeSyntheticPack(directory: string, count: number) {
  const source = loadSourceCorpus(repoRoot);
  const random = mulberry32(1);
  mkdirSync(path.join(directory, "bodies"), { recursive: true });
  const entries = [];
  for (let index = 0; index < count; index += 1) {
    const name = `بنڈل ${String(index + 1).padStart(3, "0")}`;
    const built = buildDocument({
      repoRoot,
      pages: pickInt(random, 1, 5),
      pageOffset: pickInt(random, 0, Math.max(0, source.pages.length - 1)),
      extraFieldsPerPage: 1,
      label: name,
    });
    const json = Buffer.from(JSON.stringify(built.envelope));
    const bodyHash = createHash("sha256").update(json).digest("hex");
    writeFileSync(
      path.join(directory, templatePackBodyPath(bodyHash)),
      gzipSync(json, { level: 6 }),
    );
    entries.push({ name, bodyHash, bytes: json.byteLength });
  }
  const packHash = createHash("sha256").update(JSON.stringify(entries)).digest("hex");
  writeFileSync(
    path.join(directory, "manifest.json"),
    `${JSON.stringify({ packHash, entries }, null, 2)}\n`,
  );
}

function buildSeed(staging: string) {
  const result = spawnSync(
    process.execPath,
    [
      "--experimental-strip-types",
      "--import",
      path.join(repoRoot, "scripts/performance/register.mjs"),
      path.join(repoRoot, "scripts/template-pack/seed.ts"),
      staging,
    ],
    { cwd: repoRoot, stdio: "inherit" },
  );
  if (result.status !== 0) throw new Error(`Seed build failed for ${staging}`);
}

async function measurePack(name: string, pack: string) {
  const manifest = JSON.parse(readFileSync(path.join(pack, "manifest.json"), "utf8")) as {
    entries: readonly unknown[];
    seed: { bytes: number; sha256: string };
  };
  const installerPack = path.join(tmpdir(), `missal-fresh-installer-${name}`);
  rmSync(installerPack, { recursive: true, force: true });
  cpSync(pack, installerPack, { recursive: true });
  rmSync(path.join(installerPack, "seed.sqlite.gz"), { force: true });
  const installerManifest = JSON.parse(
    readFileSync(path.join(installerPack, "manifest.json"), "utf8"),
  ) as {
    seed?: unknown;
  };
  delete installerManifest.seed;
  writeFileSync(
    path.join(installerPack, "manifest.json"),
    `${JSON.stringify(installerManifest, null, 2)}\n`,
  );

  const expected = manifest.entries.length;
  for (let index = 0; index < warmup; index += 1) {
    await timeOpen(pack, expected);
    await timeOpen(installerPack, expected);
  }
  const seedMs: number[] = [];
  const installerMs: number[] = [];
  let databaseBytes = 0;
  for (let index = 0; index < samples; index += 1) {
    const seeded = await timeOpen(pack, expected);
    const installed = await timeOpen(installerPack, expected);
    seedMs.push(seeded.ms);
    installerMs.push(installed.ms);
    databaseBytes = seeded.databaseBytes;
  }
  rmSync(installerPack, { recursive: true, force: true });
  return {
    name,
    templates: expected,
    seedBytes: statSync(path.join(pack, "seed.sqlite.gz")).size,
    uncompressedBytes: manifest.seed.bytes,
    databaseBytes,
    seedSha256: manifest.seed.sha256,
    seed: summarize(seedMs),
    installer: summarize(installerMs),
  };
}

async function timeOpen(pack: string, expected: number) {
  const directory = mkdtempSync(path.join(tmpdir(), "missal-fresh-open-"));
  const databasePath = path.join(directory, "missal.sqlite");
  const started = performance.now();
  const runtime = makeStorageRuntime({
    databasePath,
    migrationsFolder,
    bundledTemplatesFolder: pack,
  });
  try {
    const listed = await runtime.runPromise(
      Effect.gen(function* () {
        const templates = yield* TemplateRepository;
        const status = yield* templates.packStatus.pipe(
          Effect.repeat({
            until: (value) => value._tag !== "Synchronizing",
            schedule: Schedule.spaced("10 millis"),
          }),
        );
        if (status._tag !== "Ready") throw new Error(`pack status ${status._tag}`);
        return yield* templates.list;
      }),
    );
    if (listed.length !== expected) {
      throw new Error(`listed ${listed.length}, expected ${expected}`);
    }
    return { ms: performance.now() - started, databaseBytes: statSync(databasePath).size };
  } finally {
    await runtime.dispose();
    rmSync(directory, { recursive: true, force: true });
  }
}

function summarize(samplesMs: number[]) {
  return { samplesMs, medianMs: median(samplesMs), p95Ms: percentile(samplesMs, 95) };
}

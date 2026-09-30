// `new Date()` reads the system clock, not `Date.now()`, and Drizzle stamps migrations that way.
import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pipeline } from "node:stream/promises";
import { backup, DatabaseSync } from "node:sqlite";
import { createGzip } from "node:zlib";
import { Effect, Schedule } from "effect";
import { seedHasCurrentMigrations } from "#/electron/fresh-profile";
import { makeStorageRuntime } from "#/electron/storage-runtime";
import { TEMPLATE_PACK_SEED } from "#/lib/bundled-templates";
import { TemplateRepository } from "#/repositories/index";

const SEED_EPOCH_MS = 1_700_000_000_000;

const stagingFolder = process.argv[2];
if (!stagingFolder) throw new Error("Pass the staging folder.");

const root = path.resolve(import.meta.dirname, "../..");
const migrationsFolder = path.join(root, "drizzle");
const manifestPath = path.join(stagingFolder, "manifest.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as {
  packHash: string;
  entries: readonly { name: string }[];
};
const directory = await mkdtemp(path.join(tmpdir(), "missal-seed-"));
const databasePath = path.join(directory, "missal.sqlite");
const snapshotPath = path.join(directory, "snapshot.sqlite");

try {
  const listed = await withFrozenClock(SEED_EPOCH_MS, async () => {
    const runtime = makeStorageRuntime({
      databasePath,
      migrationsFolder,
      bundledTemplatesFolder: stagingFolder,
    });
    try {
      return await runtime.runPromise(
        Effect.gen(function* () {
          const templates = yield* TemplateRepository;
          const status = yield* templates.packStatus.pipe(
            Effect.repeat({
              until: (value) => value._tag !== "Synchronizing",
              schedule: Schedule.spaced("20 millis"),
            }),
          );
          if (status._tag !== "Ready") {
            throw new Error(
              status._tag === "PartialFailure" ? status.message : "Template pack did not install",
            );
          }
          return yield* templates.list;
        }),
      );
    } finally {
      await runtime.dispose();
    }
  });
  if (listed.length !== manifest.entries.length) {
    throw new Error(
      `Seed installed ${listed.length} templates, expected ${manifest.entries.length}`,
    );
  }

  const source = new DatabaseSync(databasePath);
  try {
    await backup(source, snapshotPath);
  } finally {
    source.close();
  }

  const snapshot = new DatabaseSync(snapshotPath);
  try {
    snapshot.exec("PRAGMA wal_checkpoint(TRUNCATE)");
    snapshot.exec("PRAGMA journal_mode = DELETE");
    snapshot.exec("VACUUM");
    const state = snapshot.prepare("SELECT pack_hash AS packHash FROM template_pack_state").get();
    const installed = snapshot.prepare("SELECT count(*) AS count FROM templates").get();
    if (state?.packHash !== manifest.packHash) {
      throw new Error("Seed pack hash does not match the manifest");
    }
    if (Number(installed?.count) !== manifest.entries.length) {
      throw new Error("Seed template count does not match the manifest");
    }
  } finally {
    snapshot.close();
  }
  await rm(`${snapshotPath}-wal`, { force: true });
  await rm(`${snapshotPath}-shm`, { force: true });
  if (!seedHasCurrentMigrations(snapshotPath, migrationsFolder)) {
    throw new Error("Seed was built from stale migrations");
  }

  const hash = createHash("sha256");
  let bytes = 0;
  await pipeline(
    createReadStream(snapshotPath),
    async function* (chunks: AsyncIterable<Uint8Array>) {
      for await (const chunk of chunks) {
        bytes += chunk.byteLength;
        hash.update(chunk);
        yield chunk;
      }
    },
    createGzip({ level: 6 }),
    createWriteStream(path.join(stagingFolder, TEMPLATE_PACK_SEED)),
  );
  await writeFile(
    manifestPath,
    `${JSON.stringify(
      {
        packHash: manifest.packHash,
        seed: { bytes, sha256: hash.digest("hex") },
        entries: manifest.entries,
      },
      null,
      2,
    )}\n`,
  );
  console.log(`Wrote ${TEMPLATE_PACK_SEED} (${bytes} bytes)`);
} finally {
  await rm(directory, { recursive: true, force: true });
}

async function withFrozenClock<T>(millis: number, run: () => Promise<T>) {
  const OriginalDate = Date;
  class FrozenDate extends OriginalDate {
    constructor(...args: unknown[]) {
      if (args.length === 0) super(millis);
      else super(...(args as ConstructorParameters<typeof Date>));
    }
  }
  FrozenDate.now = () => millis;
  globalThis.Date = FrozenDate as DateConstructor;
  try {
    return await run();
  } finally {
    globalThis.Date = OriginalDate;
  }
}

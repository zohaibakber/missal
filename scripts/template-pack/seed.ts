import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pipeline } from "node:stream/promises";
import { backup, DatabaseSync } from "node:sqlite";
import { createGzip } from "node:zlib";
import { Effect, Schedule } from "effect";
import { makeStorageRuntime } from "#/electron/storage-runtime";
import { TEMPLATE_PACK_SEED } from "#/lib/bundled-templates";
import { TemplateRepository } from "#/repositories/index";
import { withFrozenClock } from "../performance/fixture.ts";

const SEED_EPOCH_MS = 1_700_000_000_000;

const stagingFolder = process.argv[2];
if (!stagingFolder) throw new Error("Pass the staging folder.");

const root = path.resolve(import.meta.dirname, "../..");
const migrationsFolder = path.join(root, "drizzle");
const manifestPath = path.join(stagingFolder, "manifest.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as object;
const directory = await mkdtemp(path.join(tmpdir(), "missal-seed-"));
const databasePath = path.join(directory, "missal.sqlite");
const snapshotPath = path.join(directory, "snapshot.sqlite");

try {
  // Fixed so rebuilding an unchanged pack produces the same seed.
  await withFrozenClock(SEED_EPOCH_MS, async () => {
    const runtime = makeStorageRuntime({
      databasePath,
      migrationsFolder,
      bundledTemplatesFolder: stagingFolder,
    });
    try {
      const status = await runtime.runPromise(
        Effect.flatMap(TemplateRepository, (templates) =>
          templates.packStatus.pipe(
            Effect.repeat({
              until: (value) => value._tag !== "Synchronizing",
              schedule: Schedule.spaced("20 millis"),
            }),
          ),
        ),
      );
      if (status._tag !== "Ready") throw new Error("The template pack did not install");
    } finally {
      await runtime.dispose();
    }
  });

  const source = new DatabaseSync(databasePath);
  try {
    await backup(source, snapshotPath);
  } finally {
    source.close();
  }
  const snapshot = new DatabaseSync(snapshotPath);
  try {
    snapshot.exec("PRAGMA journal_mode = DELETE");
    snapshot.exec("VACUUM");
  } finally {
    snapshot.close();
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
        ...manifest,
        seed: { bytes, sha256: hash.digest("hex") },
      },
      null,
      2,
    )}\n`,
  );
  console.log(`Wrote ${TEMPLATE_PACK_SEED} (${bytes} bytes)`);
} finally {
  await rm(directory, { recursive: true, force: true });
}

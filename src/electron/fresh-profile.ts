import { createHash } from "node:crypto";
import {
  createReadStream,
  createWriteStream,
  existsSync,
  readFileSync,
  readdirSync,
} from "node:fs";
import { lstat, mkdir, open, readFile, rename, rm } from "node:fs/promises";
import path from "node:path";
import { pipeline } from "node:stream/promises";
import { DatabaseSync } from "node:sqlite";
import { createGunzip } from "node:zlib";
import { Schema } from "effect";
import type { StorageWorkerConfig } from "#/electron/storage-rpc";
import {
  TEMPLATE_PACK_MANIFEST,
  TEMPLATE_PACK_SEED,
  TemplatePackManifest,
  type TemplatePackSeed,
} from "#/lib/bundled-templates";

const PARTIAL_SUFFIX = ".partial";
const LOCK_NAME = "missal.bootstrap.lock";

/** Returns a warning when a present seed could not be published. */
export async function publishFreshProfile(
  config: StorageWorkerConfig,
): Promise<string | undefined> {
  const folder = config.bundledTemplatesFolder;
  if (folder === undefined) return undefined;

  const directory = path.dirname(config.databasePath);
  await mkdir(directory, { recursive: true });
  // Acquired before the existence check, and released by the OS if this process dies.
  const lock = new DatabaseSync(path.join(directory, LOCK_NAME));
  try {
    lock.exec("PRAGMA busy_timeout = 30000");
    lock.exec("PRAGMA journal_mode = DELETE");
    lock.exec("BEGIN EXCLUSIVE");
    try {
      return await publishUnderLock(config, folder);
    } finally {
      lock.exec("ROLLBACK");
    }
  } finally {
    lock.close();
  }
}

async function publishUnderLock(config: StorageWorkerConfig, folder: string) {
  const partial = `${config.databasePath}${PARTIAL_SUFFIX}`;
  await removePartial(partial);
  if (await pathExists(config.databasePath)) return undefined;

  const seed = await readSeedRecord(folder);
  if (seed === undefined) return undefined;

  try {
    await publishSeed(config, folder, seed, partial);
    return undefined;
  } catch (error) {
    await removePartial(partial);
    return error instanceof Error ? error.message : "seed verification failed";
  }
}

async function publishSeed(
  config: StorageWorkerConfig,
  folder: string,
  seed: TemplatePackSeed,
  partial: string,
) {
  const hash = createHash("sha256");
  let bytes = 0;
  await pipeline(
    createReadStream(path.join(folder, TEMPLATE_PACK_SEED)),
    createGunzip(),
    async function* (chunks: AsyncIterable<Uint8Array>) {
      for await (const chunk of chunks) {
        bytes += chunk.byteLength;
        if (bytes > seed.bytes) throw new Error("seed is larger than the manifest");
        hash.update(chunk);
        yield chunk;
      }
    },
    createWriteStream(partial, { flags: "wx" }),
  );
  if (bytes !== seed.bytes || hash.digest("hex") !== seed.sha256) {
    throw new Error("seed hash does not match the manifest");
  }
  if (!seedHasCurrentMigrations(partial, config.migrationsFolder)) {
    throw new Error("seed schema does not include the current migrations");
  }

  const handle = await open(partial, "r+");
  await handle.sync();
  await handle.close();
  if (await pathExists(config.databasePath)) {
    await removePartial(partial);
    return;
  }
  await rename(partial, config.databasePath);
}

async function readSeedRecord(folder: string) {
  try {
    const manifest = Schema.decodeUnknownSync(TemplatePackManifest)(
      JSON.parse(await readFile(path.join(folder, TEMPLATE_PACK_MANIFEST), "utf8")) as unknown,
    );
    return manifest.seed;
  } catch {
    return undefined;
  }
}

export function seedHasCurrentMigrations(databasePath: string, migrationsFolder: string) {
  const expected = migrationHashes(migrationsFolder);
  if (expected.length === 0) return false;
  const database = new DatabaseSync(databasePath, { readOnly: true });
  try {
    const rows = database.prepare("SELECT hash FROM __drizzle_migrations").all();
    const have = new Set(rows.map((row) => String(row.hash)));
    return expected.every((hash) => have.has(hash));
  } catch {
    return false;
  } finally {
    database.close();
  }
}

async function pathExists(file: string) {
  return lstat(file).then(
    () => true,
    (error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT") return false;
      throw error;
    },
  );
}

async function removePartial(partial: string) {
  await rm(partial, { force: true });
  await rm(`${partial}-wal`, { force: true });
  await rm(`${partial}-shm`, { force: true });
}

function migrationHashes(migrationsFolder: string) {
  return readdirSync(migrationsFolder)
    .filter((name) => existsSync(path.join(migrationsFolder, name, "migration.sql")))
    .sort()
    .map((name) =>
      createHash("sha256")
        .update(readFileSync(path.join(migrationsFolder, name, "migration.sql")))
        .digest("hex"),
    );
}

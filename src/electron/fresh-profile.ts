import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
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
} from "#/lib/bundled-templates";

/**
 * Copies the bundled seed database into a profile that has no database yet. An existing file,
 * even a corrupt one, is never replaced.
 */
export async function publishFreshProfile(config: StorageWorkerConfig) {
  const folder = config.bundledTemplatesFolder;
  if (folder === undefined) return;

  const directory = path.dirname(config.databasePath);
  await mkdir(directory, { recursive: true });
  // Taken before the existence check so two launches cannot both publish; the OS releases it
  // if this process dies.
  const lock = new DatabaseSync(path.join(directory, "missal.bootstrap.lock"));
  try {
    lock.exec("PRAGMA busy_timeout = 30000");
    lock.exec("BEGIN EXCLUSIVE");
    const partial = `${config.databasePath}.partial`;
    await rm(partial, { force: true });
    if (await exists(config.databasePath)) return;
    try {
      await publishSeed(folder, partial, config.databasePath);
    } catch (error) {
      await rm(partial, { force: true });
      throw error;
    }
  } finally {
    lock.close();
  }
}

async function publishSeed(folder: string, partial: string, databasePath: string) {
  const manifest = Schema.decodeUnknownSync(TemplatePackManifest)(
    JSON.parse(await readFile(path.join(folder, TEMPLATE_PACK_MANIFEST), "utf8")) as unknown,
  );
  const seed = manifest.seed;
  if (seed === undefined) return;

  const hash = createHash("sha256");
  let bytes = 0;
  await pipeline(
    createReadStream(path.join(folder, TEMPLATE_PACK_SEED)),
    createGunzip(),
    async function* (chunks: AsyncIterable<Uint8Array>) {
      for await (const chunk of chunks) {
        bytes += chunk.byteLength;
        if (bytes > seed.bytes) throw new Error("The seed database is larger than its manifest");
        hash.update(chunk);
        yield chunk;
      }
    },
    createWriteStream(partial, { flags: "wx" }),
  );
  if (bytes !== seed.bytes || hash.digest("hex") !== seed.sha256) {
    throw new Error("The seed database does not match its manifest");
  }
  const handle = await open(partial, "r+");
  await handle.sync();
  await handle.close();
  await rename(partial, databasePath);
}

const exists = (file: string) =>
  lstat(file).then(
    () => true,
    (error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT") return false;
      throw error;
    },
  );

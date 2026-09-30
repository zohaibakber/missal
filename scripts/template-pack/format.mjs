// Pack hash key order is the contract shared with templatePackHash in
// src/electron/bundled-templates.ts. Do not reorder keys.
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { gunzipSync } from "node:zlib";

export const TEMPLATE_PACK_FORMAT_VERSION = 2;
export const TEMPLATE_PACK_SCHEMA_VERSION = 1;
export const TEMPLATE_PACK_GZIP_LEVEL = 6;
export const TEMPLATE_PACK_MANIFEST = "manifest.json";

export function templatePackHash(manifest) {
  return createHash("sha256")
    .update(
      JSON.stringify({
        converterVersion: manifest.converterVersion,
        entries: manifest.entries.map((entry) => ({
          bodyHash: entry.bodyHash,
          decodedBytes: entry.decodedBytes,
          displayName: entry.displayName,
          fieldNames: [...entry.fieldNames],
          name: entry.name,
          sourceHash: entry.sourceHash,
        })),
        formatVersion: manifest.formatVersion,
        schemaVersion: manifest.schemaVersion,
      }),
    )
    .digest("hex");
}

export function validatePack(folder) {
  const manifestPath = path.join(folder, TEMPLATE_PACK_MANIFEST);
  if (!existsSync(manifestPath)) return ["missing manifest"];
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const errors = [];
  if (manifest.formatVersion !== TEMPLATE_PACK_FORMAT_VERSION) errors.push("format version");
  if (manifest.schemaVersion !== TEMPLATE_PACK_SCHEMA_VERSION) errors.push("schema version");
  if (!Array.isArray(manifest.entries)) return [...errors, "entries"];
  if (templatePackHash(manifest) !== manifest.packHash) errors.push("pack hash");
  const names = new Set();
  for (const entry of manifest.entries) {
    if (names.has(entry.name)) errors.push(`duplicate identity ${entry.name}`);
    names.add(entry.name);
    const bodyPath = path.join(folder, "bodies", `${entry.bodyHash}.json.gz`);
    if (!existsSync(bodyPath)) {
      errors.push(`missing body ${entry.name}`);
      continue;
    }
    const json = gunzipSync(readFileSync(bodyPath));
    const bodyHash = createHash("sha256").update(json).digest("hex");
    if (bodyHash !== entry.bodyHash) errors.push(`body hash ${entry.name}`);
    if (json.byteLength !== entry.decodedBytes) errors.push(`decoded size ${entry.name}`);
  }
  return errors;
}

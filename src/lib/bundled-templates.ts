import { Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";
import { FieldReference } from "#/lib/field";
import { normalizePlaceholderName } from "#/lib/placeholder";
import { NonEmptyTrimmedString } from "#/lib/schema";

export const TEMPLATE_PACK_FORMAT_VERSION = 2 as const;
export const TEMPLATE_PACK_SCHEMA_VERSION = 1 as const;
/** Bump when conversion changes an envelope without changing the Word file. */
export const TEMPLATE_CONVERTER_VERSION = 1;
export const TEMPLATE_PACK_MANIFEST = "manifest.json";
export const TEMPLATE_PACK_BATCH_TEMPLATES = 4;
export const TEMPLATE_PACK_BATCH_BYTES = 4 * 1024 * 1024;
export const TEMPLATE_PACK_MAX_DOCUMENT_BYTES = 4 * 1024 * 1024;

const Sha256 = Schema.String.pipe(Schema.check(Schema.isPattern(/^[a-f0-9]{64}$/)));
const ByteCount = Schema.Int.pipe(Schema.check(Schema.isGreaterThanOrEqualTo(0)));

export class TemplatePackEntry extends Schema.Class<TemplatePackEntry>("TemplatePackEntry")({
  /** Source-file identity. Renaming the Word file still installs a new template. */
  name: NonEmptyTrimmedString,
  displayName: NonEmptyTrimmedString,
  /** SHA-256 of the uncompressed envelope JSON. */
  bodyHash: Sha256,
  /** SHA-256 of the Word file the envelope was converted from. */
  sourceHash: Sha256,
  decodedBytes: ByteCount,
  fieldNames: Schema.Array(NonEmptyTrimmedString),
}) {}

export class TemplatePackManifest extends Schema.Class<TemplatePackManifest>(
  "TemplatePackManifest",
)({
  formatVersion: Schema.Literal(TEMPLATE_PACK_FORMAT_VERSION),
  schemaVersion: Schema.Literal(TEMPLATE_PACK_SCHEMA_VERSION),
  converterVersion: Schema.Int.pipe(Schema.check(Schema.isGreaterThanOrEqualTo(1))),
  packHash: Sha256,
  entries: Schema.Array(TemplatePackEntry),
}) {}

export const TemplatePackIdle = Schema.TaggedStruct("Idle", {});
export const TemplatePackSynchronizing = Schema.TaggedStruct("Synchronizing", {
  done: ByteCount,
  total: ByteCount,
});
export const TemplatePackReady = Schema.TaggedStruct("Ready", {});
export const TemplatePackPartialFailure = Schema.TaggedStruct("PartialFailure", {
  message: Schema.String,
});
export const TemplatePackStatus = Schema.Union([
  TemplatePackIdle,
  TemplatePackSynchronizing,
  TemplatePackReady,
  TemplatePackPartialFailure,
]);
export type TemplatePackStatus = typeof TemplatePackStatus.Type;

export function templatePackBodyPath(bodyHash: string) {
  return `bodies/${bodyHash}.json.gz`;
}

/** The field name a Word merge field stands for: `چالانی_ضمنی_نمبر_` is "چالانی ضمنی نمبر". */
export function fieldNameFromToken(token: string) {
  return normalizePlaceholderName(token.replaceAll("_", " "));
}

/**
 * Rewrites every field reference in a document. Packs store fields by name because field IDs
 * differ between installs.
 */
export function mapDocumentFields(
  envelope: DocumentEnvelope,
  map: (reference: FieldReference) => FieldReference,
) {
  return new DocumentEnvelope({
    format: envelope.format,
    version: envelope.version,
    state: mapFieldReferences(envelope.state, map),
    ...(envelope.pageLayout ? { pageLayout: envelope.pageLayout } : {}),
  });
}

function mapFieldReferences(
  state: unknown,
  map: (reference: FieldReference) => FieldReference,
): unknown {
  if (Array.isArray(state)) return state.map((child) => mapFieldReferences(child, map));
  if (typeof state !== "object" || state === null) return state;
  const node = Object.fromEntries(
    Object.entries(state).map(([key, value]) => [key, mapFieldReferences(value, map)]),
  );
  if (node.type === "field") {
    const reference = Schema.decodeUnknownOption(FieldReference)(node.reference);
    if (reference._tag === "Some") node.reference = map(reference.value);
  }
  return node;
}

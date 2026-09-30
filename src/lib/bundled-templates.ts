import { Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";
import { FieldReference } from "#/lib/field";
import { normalizePlaceholderName } from "#/lib/placeholder";
import { NonEmptyTrimmedString } from "#/lib/schema";

export const TEMPLATE_PACK_MANIFEST = "manifest.json";
export const TEMPLATE_PACK_BATCH_TEMPLATES = 4;
/** Uncompressed bytes decoded per batch; a larger single document is rejected unread. */
export const TEMPLATE_PACK_BATCH_BYTES = 4 * 1024 * 1024;

export class TemplatePackEntry extends Schema.Class<TemplatePackEntry>("TemplatePackEntry")({
  /** Source-file identity. Renaming the Word file installs a new template. */
  name: NonEmptyTrimmedString,
  /** SHA-256 of the uncompressed envelope JSON, and the body's file name. */
  bodyHash: Schema.String,
  bytes: Schema.Int,
}) {}

export class TemplatePackManifest extends Schema.Class<TemplatePackManifest>(
  "TemplatePackManifest",
)({
  packHash: Schema.String,
  entries: Schema.Array(TemplatePackEntry),
}) {}

export const TemplatePackIdle = Schema.TaggedStruct("Idle", {});
export const TemplatePackSynchronizing = Schema.TaggedStruct("Synchronizing", {
  done: Schema.Int,
  total: Schema.Int,
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

import { Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";
import { FieldReference } from "#/lib/field";
import { normalizePlaceholderName } from "#/lib/placeholder";
import { NonEmptyTrimmedString } from "#/lib/schema";

/** Lists the templates in a pack folder; each one's document is stored in `<name>.json`. */
export const BUNDLED_TEMPLATES_INDEX = "index.json";

export class BundledTemplateEntry extends Schema.Class<BundledTemplateEntry>(
  "BundledTemplateEntry",
)({
  name: NonEmptyTrimmedString,
  /** SHA-256 of the Word file the document was converted from. */
  sourceHash: Schema.String,
}) {}

export const BundledTemplateIndex = Schema.Array(BundledTemplateEntry);

export function bundledTemplateFileName(name: string) {
  return `${name}.json`;
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

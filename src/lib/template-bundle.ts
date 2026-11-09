import { Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";
import { FIR_FIELDS } from "#/lib/field";
import type { PlaceholderId } from "#/lib/ids";
import { fieldLookupKey } from "#/lib/placeholder";
import { NonEmptyTrimmedString } from "#/lib/schema";

export const TemplateBundleFields = Schema.Struct({
  version: Schema.Int.pipe(Schema.check(Schema.isGreaterThan(0))),
  global: Schema.Array(NonEmptyTrimmedString),
  custom: Schema.Array(NonEmptyTrimmedString),
});

export type TemplateBundleFields = typeof TemplateBundleFields.Type;

export const BundledTemplates = Schema.Array(
  Schema.Struct({
    name: NonEmptyTrimmedString,
    document: DocumentEnvelope,
  }),
);

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };

export function mapReferences(
  value: JsonValue,
  map: (reference: JsonObject) => JsonValue,
): JsonValue {
  if (Array.isArray(value)) return value.map((item) => mapReferences(item, map));
  if (value === null || typeof value !== "object") return value;
  const result: JsonObject = {};
  for (const [key, child] of Object.entries(value)) {
    result[key] =
      key === "reference" && child !== null && typeof child === "object" && !Array.isArray(child)
        ? map(child)
        : mapReferences(child, map);
  }
  return result;
}

export function unbindCatalogFields(
  state: unknown,
  labelOf: (id: number) => string | undefined,
): unknown {
  return mapReferences(state as JsonValue, (reference) => {
    const label =
      reference._tag === "CatalogField" && typeof reference.id === "number"
        ? labelOf(reference.id)
        : undefined;
    return label ? { _tag: "UnresolvedToken", text: label } : reference;
  });
}

const firFieldsByName = new Map(
  FIR_FIELDS.map(({ label, reference }) => [fieldLookupKey(label), reference]),
);

export function bindCatalogFields(
  state: unknown,
  idOf: ReadonlyMap<string, PlaceholderId>,
): unknown {
  return mapReferences(state as JsonValue, (reference) => {
    if (reference._tag !== "UnresolvedToken" || typeof reference.text !== "string") {
      return reference;
    }
    const key = fieldLookupKey(reference.text);
    const firField = firFieldsByName.get(key);
    if (firField) return { ...firField };
    const id = idOf.get(key);
    return id === undefined ? reference : { _tag: "CatalogField", id };
  });
}

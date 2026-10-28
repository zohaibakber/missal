import { Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";
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

type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
type JsonObject = { [key: string]: JsonValue };

function mapReferences(value: JsonValue, map: (reference: JsonObject) => JsonValue): JsonValue {
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

export function bindCatalogFields(
  state: unknown,
  idOf: ReadonlyMap<string, PlaceholderId>,
): unknown {
  return mapReferences(state as JsonValue, (reference) => {
    const id =
      reference._tag === "UnresolvedToken" && typeof reference.text === "string"
        ? idOf.get(fieldLookupKey(reference.text))
        : undefined;
    return id === undefined ? reference : { _tag: "CatalogField", id };
  });
}

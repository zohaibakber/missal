import { HashMap, Option, Schema, SchemaTransformation } from "effect";
import {
  CatalogFieldReference,
  FIR_FIELDS,
  FieldSource,
  UnresolvedTokenReference,
  type FieldReference,
} from "#/lib/field";
import { PlaceholderId } from "#/lib/ids";

export { PlaceholderId } from "#/lib/ids";

export function normalizePlaceholderName(name: string) {
  return name.trim().replace(/\s+/g, " ");
}

export function fieldLookupKey(name: string) {
  return name.replace(/[\s_\u200c]+/g, "");
}

const firFieldNames = new Set(FIR_FIELDS.map(({ label }) => fieldLookupKey(label)));

export const PlaceholderName = Schema.String.pipe(
  Schema.decode(
    SchemaTransformation.transform({ decode: normalizePlaceholderName, encode: (name) => name }),
  ),
  Schema.check(Schema.isNonEmpty()),
  Schema.check(
    Schema.makeFilter(
      (name) => !firFieldNames.has(fieldLookupKey(name)) || "This name belongs to an FIR field",
    ),
  ),
);

export class Placeholder extends Schema.Class<Placeholder>("Placeholder")({
  id: PlaceholderId,
  label: PlaceholderName,
  source: FieldSource,
}) {}

export class PlaceholderCreateInput extends Schema.Class<PlaceholderCreateInput>(
  "PlaceholderCreateInput",
)({
  label: PlaceholderName,
}) {}

export class PlaceholderUpdateInput extends Schema.Class<PlaceholderUpdateInput>(
  "PlaceholderUpdateInput",
)({
  id: PlaceholderId,
  label: PlaceholderName,
}) {}

export type FieldIndex = {
  byId: HashMap.HashMap<Placeholder["id"], Placeholder>;
  byName: HashMap.HashMap<string, FieldReference>;
};

export function indexFields(placeholders: readonly Placeholder[]): FieldIndex {
  return {
    byId: HashMap.fromIterable(placeholders.map((placeholder) => [placeholder.id, placeholder])),
    byName: HashMap.fromIterable<string, FieldReference>([
      ...FIR_FIELDS.map(({ label, reference }) => [fieldLookupKey(label), reference] as const),
      ...placeholders.map(
        (placeholder) =>
          [
            fieldLookupKey(placeholder.label),
            CatalogFieldReference.make({ id: placeholder.id }),
          ] as const,
      ),
    ]),
  };
}

export function resolveField(token: string, index: FieldIndex) {
  const key = fieldLookupKey(token);
  return key ? Option.getOrUndefined(HashMap.get(index.byName, key)) : undefined;
}

export function resolveFieldReference(token: string, index: FieldIndex): FieldReference {
  return (
    resolveField(token, index) ??
    UnresolvedTokenReference.make({ text: normalizePlaceholderName(token) })
  );
}

import { HashMap, Option, Schema, SchemaTransformation } from "effect";
import {
  CatalogFieldReference,
  FieldSource,
  UnresolvedTokenReference,
  fieldSourceForSeedKey,
  type FieldReference,
} from "#/lib/field";
import { PlaceholderId } from "#/lib/ids";
import { NonEmptyTrimmedString } from "#/lib/schema";

export { PlaceholderId } from "#/lib/ids";

export const PlaceholderKey = Schema.String.pipe(
  Schema.decode(SchemaTransformation.trim()),
  Schema.check(Schema.isNonEmpty()),
  Schema.check(Schema.isPattern(/^[A-Za-z][A-Za-z0-9_]*$/)),
  Schema.brand("PlaceholderKey"),
);

export type PlaceholderKey = typeof PlaceholderKey.Type;

export class Placeholder extends Schema.Class<Placeholder>("Placeholder")({
  id: PlaceholderId,
  key: PlaceholderKey,
  label: NonEmptyTrimmedString,
  source: FieldSource,
}) {}

export class PlaceholderCreateInput extends Schema.Class<PlaceholderCreateInput>(
  "PlaceholderCreateInput",
)({
  label: NonEmptyTrimmedString,
}) {}

export class PlaceholderUpdateInput extends Schema.Class<PlaceholderUpdateInput>(
  "PlaceholderUpdateInput",
)({
  id: PlaceholderId,
  key: PlaceholderKey,
  label: NonEmptyTrimmedString,
}) {}

export type PlaceholderIndex = {
  byId: HashMap.HashMap<Placeholder["id"], Placeholder>;
  byKey: HashMap.HashMap<string, Placeholder>;
  byLabel: HashMap.HashMap<string, Placeholder>;
};

const defaultPlaceholderSeeds = [
  { key: "fir_no", label: "ایف آئی آر نمبر" },
  { key: "date", label: "تاریخ ایف آئی آر" },
  { key: "incident_date", label: "تاریخ وقوعہ" },
  { key: "arrest_date", label: "تاریخ گرفتاری" },
  { key: "offence", label: "جرم" },
  { key: "accused", label: "نام ملزم و سکونت" },
  { key: "witness", label: "گواہان" },
  { key: "nic", label: "شناختی کارڈ" },
  { key: "mobile", label: "موبائل" },
  { key: "investigation_officer", label: "تفتیشی افسر" },
  { key: "police_station", label: "تھانہ نام" },
  { key: "district", label: "ضلع نام" },
  { key: "sho_name", label: "SHO نام" },
  { key: "dsp_name", label: "DSP نام" },
  { key: "zimni", label: "ضمنی" },
] as const;

export function indexPlaceholders(placeholders: readonly Placeholder[]): PlaceholderIndex {
  return {
    byId: HashMap.fromIterable(placeholders.map((placeholder) => [placeholder.id, placeholder])),
    byKey: HashMap.fromIterable(
      placeholders.map((placeholder) => [placeholder.key, placeholder] as const),
    ),
    byLabel: HashMap.fromIterable(
      placeholders.map((placeholder) => [placeholder.label.trim(), placeholder] as const),
    ),
  };
}

export function createDefaultPlaceholders() {
  return defaultPlaceholderSeeds.map(
    (placeholder, index) =>
      new Placeholder({
        id: PlaceholderId.make(index + 1),
        key: PlaceholderKey.make(placeholder.key),
        label: placeholder.label,
        source: fieldSourceForSeedKey(placeholder.key),
      }),
  );
}

export function resolvePlaceholder(
  token: string,
  index: PlaceholderIndex = indexPlaceholders([]),
): Placeholder | undefined {
  const value = token.trim();

  if (!value) {
    return undefined;
  }

  if (/^\d+$/.test(value)) {
    const byId = Option.getOrUndefined(HashMap.get(index.byId, Number(value) as Placeholder["id"]));

    if (byId) {
      return byId;
    }
  }

  const byKey = Option.getOrUndefined(HashMap.get(index.byKey, value));

  if (byKey) {
    return byKey;
  }

  return Option.getOrUndefined(HashMap.get(index.byLabel, value));
}

export function resolveFieldReference(
  token: string,
  index: PlaceholderIndex = indexPlaceholders([]),
): FieldReference {
  const placeholder = resolvePlaceholder(token, index);

  if (placeholder) {
    return CatalogFieldReference.make({ id: placeholder.id });
  }

  return UnresolvedTokenReference.make({ text: token.trim() });
}

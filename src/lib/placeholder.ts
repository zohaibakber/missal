import { HashMap, Option, Schema, SchemaTransformation } from "effect";
import {
  CatalogFieldReference,
  FieldSource,
  UnresolvedTokenReference,
  fieldSourceForSeedKey,
  type FieldReference,
} from "#/lib/field";
import { PlaceholderId } from "#/lib/ids";

export { PlaceholderId } from "#/lib/ids";

/** Names match regardless of surrounding or repeated whitespace, as typed in a template. */
export function normalizePlaceholderName(name: string) {
  return name.trim().replace(/\s+/g, " ");
}

/** The one name a placeholder has: shown in the UI and typed between markers in templates. */
export const PlaceholderName = Schema.String.pipe(
  Schema.decode(
    SchemaTransformation.transform({ decode: normalizePlaceholderName, encode: (name) => name }),
  ),
  Schema.check(Schema.isNonEmpty()),
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

export type PlaceholderIndex = {
  byId: HashMap.HashMap<Placeholder["id"], Placeholder>;
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
    byLabel: HashMap.fromIterable(
      placeholders.map(
        (placeholder) => [normalizePlaceholderName(placeholder.label), placeholder] as const,
      ),
    ),
  };
}

export function createDefaultPlaceholders() {
  return defaultPlaceholderSeeds.map(
    (placeholder, index) =>
      new Placeholder({
        id: PlaceholderId.make(index + 1),
        label: placeholder.label,
        source: fieldSourceForSeedKey(placeholder.key),
      }),
  );
}

export function resolvePlaceholder(
  token: string,
  index: PlaceholderIndex = indexPlaceholders([]),
): Placeholder | undefined {
  const name = normalizePlaceholderName(token);
  return name ? Option.getOrUndefined(HashMap.get(index.byLabel, name)) : undefined;
}

export function resolveFieldReference(
  token: string,
  index: PlaceholderIndex = indexPlaceholders([]),
): FieldReference {
  const placeholder = resolvePlaceholder(token, index);

  if (placeholder) {
    return CatalogFieldReference.make({ id: placeholder.id });
  }

  return UnresolvedTokenReference.make({ text: normalizePlaceholderName(token) });
}

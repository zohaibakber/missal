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

export function normalizePlaceholderName(name: string) {
  return name.trim().replace(/\s+/g, " ");
}

/**
 * How a name is looked up. Word merge fields cannot hold spaces, so templates write
 * `«تھانہ_نام_»` or `«گواہان2»` for the fields named "تھانہ نام" and "گواہان 2".
 */
function placeholderLookupKey(name: string) {
  return name.replace(/[\s_\u200c]+/g, "");
}

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

/** Names follow the station's Word templates, which write them as `«مقدمہ_نمبر»`. */
const defaultPlaceholderSeeds: readonly { key: string; label: string; index?: number }[] = [
  { key: "fir_no", label: "مقدمہ نمبر" },
  { key: "date", label: "Date FIR" },
  { key: "incident_date", label: "تاریخ ووقت وقوعہ" },
  { key: "arrest_date", label: "تاریخ گرفتاری" },
  { key: "offence", label: "جرم" },
  { key: "accused", label: "نام ملزم و سکونت" },
  { key: "witness", label: "گواہان" },
  { key: "nic", label: "شناختی کارڈ" },
  { key: "mobile", label: "موبائل" },
  { key: "investigation_officer", label: "تفتیشی" },
  { key: "police_station", label: "تھانہ نام" },
  { key: "district", label: "ضلع نام" },
  { key: "sho_name", label: "SHO نام" },
  { key: "dsp_name", label: "DSP نام" },
  { key: "zimni", label: "ضمنی" },
  { key: "witness", label: "گواہان 1", index: 1 },
  { key: "witness", label: "گواہان 2", index: 2 },
  { key: "zimni", label: "ضمنی 1", index: 1 },
  { key: "zimni", label: "ضمنی 2", index: 2 },
  { key: "complainant", label: "مدعی مقدمہ" },
  { key: "accused_description", label: "حلیہ ملزم" },
  { key: "brief_facts", label: "مختصر حالات" },
  { key: "written_by", label: "تحریر کنندہ" },
  { key: "zimni_date", label: "تاریخ ضمنی" },
  { key: "challan_zimni_no", label: "چالانی ضمنی نمبر" },
  { key: "challan_zimni_date", label: "چالانی ضمنی تاریخ" },
  { key: "document_date", label: "تاریخ2" },
];

export function indexPlaceholders(placeholders: readonly Placeholder[]): PlaceholderIndex {
  return {
    byId: HashMap.fromIterable(placeholders.map((placeholder) => [placeholder.id, placeholder])),
    byLabel: HashMap.fromIterable(
      placeholders.map(
        (placeholder) => [placeholderLookupKey(placeholder.label), placeholder] as const,
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
        source: fieldSourceForSeedKey(placeholder.key, placeholder.index),
      }),
  );
}

export function resolvePlaceholder(
  token: string,
  index: PlaceholderIndex = indexPlaceholders([]),
): Placeholder | undefined {
  const key = placeholderLookupKey(token);
  return key ? Option.getOrUndefined(HashMap.get(index.byLabel, key)) : undefined;
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

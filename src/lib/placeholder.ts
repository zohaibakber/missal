import { HashMap, Option, Schema, SchemaTransformation } from "effect";
import type { FirRecord } from "#/lib/fir";
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
}) {}

export class PlaceholderCreateInput extends Schema.Class<PlaceholderCreateInput>(
  "PlaceholderCreateInput",
)({
  key: PlaceholderKey,
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

export const CORE_PLACEHOLDER_FIELDS = {
  fir_no: "fir_no",
  date: "date",
  incident_date: "incident_date",
  arrest_date: "arrest_date",
  offence: "offence",
  accused: "accused",
  witness: "witness",
  nic: "NIC",
  mobile: "mobile",
  investigation_officer: "investigation_officer",
} as const satisfies Record<string, keyof FirRecord>;

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
      }),
  );
}

export function isPlaceholderToken(token: string) {
  return /^\d+$/.test(token) || /^[A-Za-z][A-Za-z0-9_]*$/.test(token);
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

export function resolvePlaceholderKey(
  token: string,
  index: PlaceholderIndex = indexPlaceholders([]),
) {
  const placeholder = resolvePlaceholder(token, index);

  if (placeholder) {
    return placeholder.key;
  }

  const value = token.trim();
  return isPlaceholderToken(value) ? value : "";
}

export function resolvePlaceholderToken(token: string, index: PlaceholderIndex) {
  const placeholder = resolvePlaceholder(token, index);

  if (placeholder) {
    return String(placeholder.id);
  }

  return resolvePlaceholderKey(token, index);
}

export function isCorePlaceholderKey(key: string): key is keyof typeof CORE_PLACEHOLDER_FIELDS {
  return key in CORE_PLACEHOLDER_FIELDS;
}

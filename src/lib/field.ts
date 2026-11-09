import { HashMap, Match, Option, Schema } from "effect";
import { NonEmptyTrimmedString } from "#/lib/schema";
import type { FirRecord } from "#/lib/fir";
import { PlaceholderId } from "#/lib/ids";
import type { FieldIndex, Placeholder } from "#/lib/placeholder";

const FIR_PROPERTY_NAMES = [
  "fir_no",
  "complainant",
  "date",
  "incident_date",
  "arrest_date",
  "offence",
  "accused",
  "witness",
  "zimni",
  "investigation_officer",
  "writer",
  "recovery",
  "body",
  "imei_1",
  "imei_2",
  "sim_1",
  "sim_2",
] as const;

const FirPropertyName = Schema.Literals(FIR_PROPERTY_NAMES);

type FirPropertyName = typeof FirPropertyName.Type;

const SharedSettingKey = NonEmptyTrimmedString;

export const FirFieldReference = Schema.TaggedStruct("FirField", {
  property: FirPropertyName,
  index: Schema.optionalKey(Schema.Int.pipe(Schema.check(Schema.isGreaterThan(0)))),
});

export type FirFieldReference = typeof FirFieldReference.Type;

export const SharedSettingSource = Schema.TaggedStruct("SharedSetting", {
  setting: SharedSettingKey,
});

export type SharedSettingSource = typeof SharedSettingSource.Type;

export const CustomSource = Schema.TaggedStruct("Custom", {});

export type CustomSource = typeof CustomSource.Type;

export const FieldSource = Schema.Union([SharedSettingSource, CustomSource]);

export type FieldSource = typeof FieldSource.Type;

export const CatalogFieldReference = Schema.TaggedStruct("CatalogField", {
  id: PlaceholderId,
});

export type CatalogFieldReference = typeof CatalogFieldReference.Type;

export const UnresolvedTokenReference = Schema.TaggedStruct("UnresolvedToken", {
  text: Schema.String,
});

export type UnresolvedTokenReference = typeof UnresolvedTokenReference.Type;

export const FieldReference = Schema.Union([
  CatalogFieldReference,
  FirFieldReference,
  UnresolvedTokenReference,
]);

export type FieldReference = typeof FieldReference.Type;

export const FieldDisplayMode = Schema.Literals(["labels", "values"]);

export type FieldDisplayMode = typeof FieldDisplayMode.Type;

const firField = (label: string, property: FirPropertyName, index?: number) => ({
  label,
  reference: FirFieldReference.make(index === undefined ? { property } : { property, index }),
});

export const FIR_FIELDS = [
  firField("مقدمہ نمبر", "fir_no"),
  firField("مدعی مقدمہ", "complainant"),
  firField("Date FIR", "date"),
  firField("تاریخ ووقت وقوعہ", "incident_date"),
  firField("تاریخ گرفتاری", "arrest_date"),
  firField("جرم", "offence"),
  firField("نام ملزم و سکونت", "accused"),
  firField("نام ملزم و سکونت 1", "accused", 1),
  firField("نام ملزم و سکونت 2", "accused", 2),
  firField("گواہان", "witness"),
  firField("گواہان 1", "witness", 1),
  firField("گواہان 2", "witness", 2),
  firField("ضمنی", "zimni"),
  firField("ضمنی 1", "zimni", 1),
  firField("ضمنی 2", "zimni", 2),
  firField("ضمنی 3", "zimni", 3),
  firField("ضمنی 4", "zimni", 4),
  firField("ضمنی 5", "zimni", 5),
  firField("ضمنی 6", "zimni", 6),
  firField("تفتیشی", "investigation_officer"),
  firField("تحریر کنندہ", "writer"),
  firField("برآمدگی", "recovery"),
  firField("متن", "body"),
  firField("ایمی نمبر 1", "imei_1"),
  firField("ایمی نمبر 2", "imei_2"),
  firField("سم نمبر 1", "sim_1"),
  firField("سم نمبر 2", "sim_2"),
];

export type FieldPresentationContext = {
  catalog: FieldIndex;
  fir: FirRecord | null;
  overrideValues: HashMap.HashMap<PlaceholderId, string>;
  sharedSettings: Record<string, string>;
  displayMode: FieldDisplayMode;
};

export type FieldDisplayText = {
  readonly text: string;
  readonly unresolved: boolean;
};

export function overrideValuesFrom(
  overrides: readonly {
    readonly placeholderId: PlaceholderId;
    readonly value: string;
  }[],
): HashMap.HashMap<PlaceholderId, string> {
  return HashMap.fromIterable(
    overrides.map((override) => [override.placeholderId, override.value] as const),
  );
}

export function catalogFieldPresentation(
  catalog: FieldIndex,
  displayMode: FieldDisplayMode = "labels",
): FieldPresentationContext {
  return {
    catalog,
    displayMode,
    fir: null,
    overrideValues: HashMap.empty<PlaceholderId, string>(),
    sharedSettings: {},
  };
}

const firFieldLabel = ({ property, index }: FirFieldReference) =>
  FIR_FIELDS.find(({ reference }) => reference.property === property && reference.index === index)
    ?.label ?? property;

export function fieldLabel(reference: FieldReference, catalog: FieldIndex) {
  return Match.valueTags(reference, {
    UnresolvedToken: ({ text }) => text,
    FirField: firFieldLabel,
    CatalogField: ({ id }) => Option.getOrUndefined(HashMap.get(catalog.byId, id))?.label,
  });
}

export function fieldDisplayText(
  reference: FieldReference,
  context: FieldPresentationContext,
): FieldDisplayText {
  return Match.valueTags(reference, {
    UnresolvedToken: ({ text }) => ({ text, unresolved: true }),
    FirField: (field) => present(firFieldLabel(field), context, (fir) => firValue(fir, field)),
    CatalogField: ({ id }) => {
      const field = Option.getOrUndefined(HashMap.get(context.catalog.byId, id));
      return field
        ? present(field.label, context, () => placeholderValue(field, context))
        : { text: String(id), unresolved: true };
    },
  });
}

function present(
  label: string,
  context: FieldPresentationContext,
  value: (fir: FirRecord) => string,
): FieldDisplayText {
  if (context.displayMode === "labels" || !context.fir) return { text: label, unresolved: false };
  const text = value(context.fir);
  return text.trim() ? { text, unresolved: false } : { text: label, unresolved: true };
}

function firValue(fir: FirRecord, { property, index }: FirFieldReference) {
  const value = fir[property];
  if (typeof value === "string") return value;
  return index === undefined ? value.join("\n") : (value[index - 1] ?? "");
}

function placeholderValue(field: Placeholder, context: FieldPresentationContext) {
  return Match.valueTags(field.source, {
    SharedSetting: ({ setting }) => context.sharedSettings[setting] ?? "",
    Custom: () => Option.getOrElse(HashMap.get(context.overrideValues, field.id), () => ""),
  });
}

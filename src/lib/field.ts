import { HashMap, Match, Option, Schema } from "effect";
import { NonEmptyTrimmedString } from "#/lib/schema";
import type { FirRecord } from "#/lib/fir";
import { PlaceholderId } from "#/lib/ids";
import type { Placeholder, PlaceholderIndex } from "#/lib/placeholder";

export const FIR_PROPERTY_NAMES = [
  "fir_no",
  "date",
  "incident_date",
  "arrest_date",
  "offence",
  "accused",
  "witness",
  "zimni",
  "NIC",
  "mobile",
  "investigation_officer",
] as const;

export const FirPropertyName = Schema.Literals(FIR_PROPERTY_NAMES);

export type FirPropertyName = typeof FirPropertyName.Type;

export const SHARED_SETTING_KEYS = [
  "investigation_officer",
  "police_station",
  "district",
  "sho_name",
  "dsp_name",
] as const;

export const SharedSettingKey = NonEmptyTrimmedString;

export type SharedSettingKey = typeof SharedSettingKey.Type;

export const PLACEHOLDER_KEY_TO_FIR_PROPERTY = {
  fir_no: "fir_no",
  date: "date",
  incident_date: "incident_date",
  arrest_date: "arrest_date",
  offence: "offence",
  accused: "accused",
  witness: "witness",
  zimni: "zimni",
  nic: "NIC",
  mobile: "mobile",
} as const satisfies Record<string, FirPropertyName>;

export const FirPropertySource = Schema.TaggedStruct("FirProperty", {
  property: FirPropertyName,
});

export type FirPropertySource = typeof FirPropertySource.Type;

export const SharedSettingSource = Schema.TaggedStruct("SharedSetting", {
  setting: SharedSettingKey,
});

export type SharedSettingSource = typeof SharedSettingSource.Type;

export const CustomSource = Schema.TaggedStruct("Custom", {});

export type CustomSource = typeof CustomSource.Type;

export const FieldSource = Schema.Union([FirPropertySource, SharedSettingSource, CustomSource]);

export type FieldSource = typeof FieldSource.Type;

export const CatalogFieldReference = Schema.TaggedStruct("CatalogField", {
  id: PlaceholderId,
});

export type CatalogFieldReference = typeof CatalogFieldReference.Type;

export const UnresolvedTokenReference = Schema.TaggedStruct("UnresolvedToken", {
  text: Schema.String,
});

export type UnresolvedTokenReference = typeof UnresolvedTokenReference.Type;

export const FieldReference = Schema.Union([CatalogFieldReference, UnresolvedTokenReference]);

export type FieldReference = typeof FieldReference.Type;

export const FieldDisplayMode = Schema.Literals(["labels", "values"]);

export type FieldDisplayMode = typeof FieldDisplayMode.Type;

export class FieldOverride extends Schema.Class<FieldOverride>("FieldOverride")({
  placeholderId: PlaceholderId,
  value: Schema.String,
}) {}

export type ResolvedFieldValue =
  | { readonly _tag: "Resolved"; readonly text: string }
  | { readonly _tag: "Unresolved"; readonly label: string };

export function fieldSourceForSeedKey(key: string): FieldSource {
  if (key in PLACEHOLDER_KEY_TO_FIR_PROPERTY) {
    return FirPropertySource.make({
      property:
        PLACEHOLDER_KEY_TO_FIR_PROPERTY[key as keyof typeof PLACEHOLDER_KEY_TO_FIR_PROPERTY],
    });
  }

  if (SHARED_SETTING_KEYS.some((setting) => setting === key)) {
    return SharedSettingSource.make({ setting: key });
  }

  return CustomSource.make({});
}

export function isSeededFieldSource(source: FieldSource) {
  return source._tag !== "Custom";
}

export type FieldPresentationContext = {
  catalog: PlaceholderIndex;
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
  catalog: PlaceholderIndex,
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

export function resolveFieldValue(
  field: Placeholder,
  fir: FirRecord,
  overrides: readonly FieldOverride[],
  sharedSettings: Record<string, string>,
): ResolvedFieldValue {
  return resolveFieldValueFromMap(field, fir, overrideValuesFrom(overrides), sharedSettings);
}

export function resolveFieldValueFromMap(
  field: Placeholder,
  fir: FirRecord,
  overrideValues: HashMap.HashMap<PlaceholderId, string>,
  sharedSettings: Record<string, string>,
): ResolvedFieldValue {
  return Match.valueTags(field.source, {
    FirProperty: ({ property }) => {
      const value = fir[property];
      return resolvedOrMissing(typeof value === "string" ? value : value.join("\n"), field.label);
    },
    SharedSetting: ({ setting }) => {
      return resolvedOrMissing(sharedSettings[setting] ?? "", field.label);
    },
    Custom: () => {
      const override = Option.getOrUndefined(HashMap.get(overrideValues, field.id));
      return resolvedOrMissing(override ?? "", field.label);
    },
  });
}

export function resolveFieldReferenceValue(
  reference: FieldReference,
  catalog: PlaceholderIndex,
  fir: FirRecord,
  overrides: readonly FieldOverride[],
  sharedSettings: Record<string, string>,
): ResolvedFieldValue {
  return resolveFieldReferenceFromMap(
    reference,
    catalog,
    fir,
    overrideValuesFrom(overrides),
    sharedSettings,
  );
}

export function fieldDisplayText(
  reference: FieldReference,
  context: FieldPresentationContext,
): FieldDisplayText {
  return Match.valueTags(reference, {
    UnresolvedToken: ({ text }) => ({ text, unresolved: true }),
    CatalogField: ({ id }) => {
      const field = Option.getOrUndefined(HashMap.get(context.catalog.byId, id));
      if (!field) {
        return { text: String(id), unresolved: true };
      }

      if (context.displayMode === "labels" || !context.fir) {
        return { text: field.label, unresolved: false };
      }

      const resolved = resolveFieldValueFromMap(
        field,
        context.fir,
        context.overrideValues,
        context.sharedSettings,
      );

      return resolved._tag === "Unresolved"
        ? { text: resolved.label, unresolved: true }
        : { text: resolved.text, unresolved: false };
    },
  });
}

function resolveFieldReferenceFromMap(
  reference: FieldReference,
  catalog: PlaceholderIndex,
  fir: FirRecord,
  overrideValues: HashMap.HashMap<PlaceholderId, string>,
  sharedSettings: Record<string, string>,
): ResolvedFieldValue {
  return Match.valueTags(reference, {
    CatalogField: ({ id }) => {
      const field = Option.getOrUndefined(HashMap.get(catalog.byId, id));
      if (!field) {
        return { _tag: "Unresolved", label: String(id) } as const;
      }

      return resolveFieldValueFromMap(field, fir, overrideValues, sharedSettings);
    },
    UnresolvedToken: ({ text }) => ({ _tag: "Unresolved", label: text }) as const,
  });
}

function resolvedOrMissing(value: string, label: string): ResolvedFieldValue {
  return value.trim() ? { _tag: "Resolved", text: value } : { _tag: "Unresolved", label };
}

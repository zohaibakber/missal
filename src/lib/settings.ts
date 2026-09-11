import { Schema } from "effect";
import { IsoDateTimeString } from "#/lib/schema";

export const SHARED_PLACEHOLDER_FIELDS = [
  {
    key: "policeStation",
    placeholderKey: "police_station",
    label: "تھانہ نام",
    placeholder: "تھانہ سٹی",
  },
  {
    key: "district",
    placeholderKey: "district",
    label: "ضلع نام",
    placeholder: "لاہور",
  },
  {
    key: "shoName",
    placeholderKey: "sho_name",
    label: "SHO نام",
    placeholder: "نام ایس ایچ او",
  },
  {
    key: "dspName",
    placeholderKey: "dsp_name",
    label: "DSP نام",
    placeholder: "نام ڈی ایس پی",
  },
] as const;

export type SharedPlaceholderKey = (typeof SHARED_PLACEHOLDER_FIELDS)[number]["key"];

export const SettingsId = Schema.Literal("default");

export const DEFAULT_SETTINGS_UPDATED_AT = IsoDateTimeString.make("1970-01-01T00:00:00.000Z");

export class AppSettings extends Schema.Class<AppSettings>("AppSettings")({
  id: SettingsId,
  sharedPlaceholders: Schema.Record(Schema.String, Schema.String),
  updatedAt: IsoDateTimeString,
}) {}

export function createDefaultAppSettings(): AppSettings {
  return new AppSettings({
    id: "default",
    sharedPlaceholders: {},
    updatedAt: DEFAULT_SETTINGS_UPDATED_AT,
  });
}

export function buildSharedPlaceholderValues(sharedPlaceholders: Record<string, string>) {
  const values: Record<string, string> = {};

  for (const field of SHARED_PLACEHOLDER_FIELDS) {
    const value = sharedPlaceholders[field.key];

    if (!value?.trim()) {
      continue;
    }

    values[field.placeholderKey] = value;
  }

  return values;
}

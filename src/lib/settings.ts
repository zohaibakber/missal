import { Schema } from "effect";
import { IsoDateTimeString } from "#/lib/schema";

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

import { Schema } from "effect";
import { IsoDateTimeString } from "#/lib/schema";

const SettingsId = Schema.Literal("default");

export class AppSettings extends Schema.Class<AppSettings>("AppSettings")({
  id: SettingsId,
  sharedPlaceholders: Schema.Record(Schema.String, Schema.String),
  updatedAt: IsoDateTimeString,
}) {}

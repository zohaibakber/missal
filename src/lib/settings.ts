import { Schema } from "effect";
import { IsoDateTimeString } from "#/lib/schema";

const SettingsId = Schema.Literal("default");

/** One to three symbols, so a marker never collides with a word or a field name. */
const FieldMarker = Schema.String.pipe(Schema.check(Schema.isPattern(/^[^\p{L}\p{N}\s]{1,3}$/u)));

/** The signs typed around a placeholder name in a template, e.g. `@جرم@`. */
export class FieldMarkers extends Schema.Class<FieldMarkers>("FieldMarkers")({
  open: FieldMarker,
  close: FieldMarker,
}) {}

export const DEFAULT_FIELD_MARKERS = new FieldMarkers({ open: "@", close: "@" });

export class AppSettings extends Schema.Class<AppSettings>("AppSettings")({
  id: SettingsId,
  sharedPlaceholders: Schema.Record(Schema.String, Schema.String),
  fieldMarkers: FieldMarkers,
  updatedAt: IsoDateTimeString,
}) {}

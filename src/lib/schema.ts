import { Schema, SchemaTransformation } from "effect";

export const NonEmptyTrimmedString = Schema.String.pipe(
  Schema.decode(SchemaTransformation.trim()),
  Schema.check(Schema.isNonEmpty()),
);

export const TrimmedString = Schema.String.pipe(Schema.decode(SchemaTransformation.trim()));

export const IsoDateTimeString = Schema.String.pipe(
  Schema.check(Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/)),
  Schema.brand("IsoDateTimeString"),
);

export type IsoDateTimeString = typeof IsoDateTimeString.Type;

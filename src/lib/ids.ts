import { Option, Schema } from "effect";

export const PlaceholderId = Schema.Int.pipe(
  Schema.check(Schema.isGreaterThanOrEqualTo(1)),
  Schema.brand("PlaceholderId"),
);

export type PlaceholderId = typeof PlaceholderId.Type;

export const TemplateId = Schema.Int.pipe(
  Schema.check(Schema.isGreaterThanOrEqualTo(1)),
  Schema.brand("TemplateId"),
);

export type TemplateId = typeof TemplateId.Type;

export const FirId = Schema.Int.pipe(
  Schema.check(Schema.isGreaterThanOrEqualTo(1)),
  Schema.brand("FirId"),
);

export type FirId = typeof FirId.Type;

function parseId<A>(schema: Schema.ConstraintDecoder<A>, value: string): A | undefined {
  return Option.getOrUndefined(Schema.decodeUnknownOption(schema)(Number(value)));
}

export function parsePlaceholderId(value: string) {
  return parseId(PlaceholderId, value);
}

export function parseTemplateId(value: string) {
  return parseId(TemplateId, value);
}

export function parseFirId(value: string) {
  return parseId(FirId, value);
}

import { Schema } from "effect";

export const EntityKind = Schema.Literals([
  "placeholder",
  "template",
  "fir",
  "firPlaceholderValue",
  "settings",
]);

export type EntityKind = typeof EntityKind.Type;

export class EntityNotFound extends Schema.TaggedError<EntityNotFound>()("EntityNotFound", {
  entity: EntityKind,
  id: Schema.String,
}) {}

export class EntityConflict extends Schema.TaggedError<EntityConflict>()("EntityConflict", {
  entity: EntityKind,
  field: Schema.String,
  value: Schema.String,
}) {}

export class StorageError extends Schema.TaggedError<StorageError>()("StorageError", {
  operation: Schema.String,
  message: Schema.String,
}) {}

export const RepositoryError = Schema.Union([EntityNotFound, EntityConflict, StorageError]);

export type RepositoryError = typeof RepositoryError.Type;

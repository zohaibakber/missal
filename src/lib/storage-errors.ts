import { Exit, Match, Option, Schema } from "effect";

export const EntityKind = Schema.Literals([
  "placeholder",
  "template",
  "fir",
  "firDocument",
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

export class EntityInUse extends Schema.TaggedError<EntityInUse>()("EntityInUse", {
  entity: EntityKind,
  id: Schema.String,
  usedBy: Schema.String,
  count: Schema.Int,
}) {}

export class StorageError extends Schema.TaggedError<StorageError>()("StorageError", {
  operation: Schema.String,
  message: Schema.String,
}) {}

export const RepositoryError = Schema.Union([
  EntityNotFound,
  EntityConflict,
  EntityInUse,
  StorageError,
]);

export type RepositoryError = typeof RepositoryError.Type;

export function getRepositoryErrorMessage(
  exit: Exit.Failure<unknown, RepositoryError>,
  fallback = "Something went wrong while saving",
) {
  return Option.match(Exit.findErrorOption(exit), {
    onNone: () => fallback,
    onSome: (error) =>
      Match.valueTags(error, {
        EntityNotFound: ({ entity }) => `${entity} not found`,
        EntityConflict: ({ field }) =>
          field === "revision"
            ? "This was saved elsewhere. Your draft is still here."
            : `${field} is already in use`,
        EntityInUse: ({ entity }) => `${entity} is in use and cannot be deleted`,
        StorageError: ({ message }) => message,
      }),
  });
}

import { Array, Cause, Effect, Match, Option, Schema } from "effect";
import { EffectDrizzleQueryError } from "drizzle-orm/effect-core/errors";
import { isSqlError, type SqlError } from "effect/sql/SqlError";
import type { MissalDrizzle } from "#/electron/database";
import { appSettings } from "#/electron/database-schema";
import { AppSettings, DEFAULT_SETTINGS } from "#/lib/settings";
import {
  EntityConflict,
  EntityNotFound,
  StorageError,
  type EntityKind,
} from "#/lib/storage-errors";

const sqlReason = (error: EffectDrizzleQueryError) =>
  Option.flatMap(
    Cause.isCause(error.cause) ? Cause.findErrorOption(error.cause) : Option.none(),
    (found) => (isSqlError(found) ? Option.some(found.reason) : Option.none()),
  );

const failStorage = (operation: string) =>
  new StorageError({
    message: "Storage operation failed",
    operation,
  });

export const failQuery = (operation: string) => (_error: EffectDrizzleQueryError) =>
  failStorage(operation);

export const mapQuery = <A, E, R>(
  operation: string,
  effect: Effect.Effect<A, E | EffectDrizzleQueryError, R>,
) =>
  Effect.catchIf(effect, Schema.is(EffectDrizzleQueryError), () =>
    Effect.fail(failStorage(operation)),
  );

export const mapTransaction = <A, E, R>(
  operation: string,
  effect: Effect.Effect<A, E | EffectDrizzleQueryError | SqlError, R>,
) =>
  Effect.catchIf(
    effect,
    (error) => Schema.is(EffectDrizzleQueryError)(error) || isSqlError(error),
    () => Effect.fail(failStorage(operation)),
  );

export const recoverUnique =
  (operation: string, entity: EntityKind, field: string, value: string) =>
  (error: EffectDrizzleQueryError): Effect.Effect<never, EntityConflict | StorageError> =>
    constraintKind(error) === "unique"
      ? uniqueConflict(entity, field, value)
      : failQuery(operation)(error);

export const decodeStored =
  <A>(decode: (value: unknown) => Effect.Effect<A, unknown>, operation: string) =>
  (value: unknown): Effect.Effect<A, StorageError> =>
    decode(value).pipe(
      Effect.mapError(
        () =>
          new StorageError({
            message: "Stored data is invalid",
            operation,
          }),
      ),
    );

export const requireRow = <A, E>(rows: readonly A[], missing: E): Effect.Effect<A, E> =>
  Option.match(Array.head(rows), {
    onNone: () => Effect.fail(missing),
    onSome: Effect.succeed,
  });

export const requireRevisionMatch = <A, E, R>(
  rows: readonly A[],
  existing: Effect.Effect<readonly unknown[], E, R>,
  entity: EntityKind,
  id: string | number,
  expectedRevision: number,
): Effect.Effect<A, E | EntityNotFound | EntityConflict, R> =>
  Option.match(Array.head(rows), {
    onNone: () =>
      existing.pipe(
        Effect.flatMap((found) => requireRow(found, notFound(entity, id))),
        Effect.flatMap(() => uniqueConflict(entity, "revision", String(expectedRevision))),
      ),
    onSome: Effect.succeed,
  });

export const notFound = (entity: EntityKind, id: string | number) =>
  new EntityNotFound({
    entity,
    id: String(id),
  });

export const uniqueConflict = (entity: EntityKind, field: string, value: string) =>
  new EntityConflict({
    entity,
    field,
    value,
  });

export const missingWrite = (operation: string) =>
  new StorageError({
    message: "Stored data was not written",
    operation,
  });

export const likeNeedle = (query: string) => {
  const trimmed = query.trim().replace(/[%_]/g, "");
  return trimmed.length === 0 ? undefined : `%${trimmed}%`;
};

export const constraintKind = (error: EffectDrizzleQueryError) =>
  Option.match(sqlReason(error), {
    onNone: () => undefined,
    onSome: (reason) =>
      Match.value(reason).pipe(
        Match.tag("UniqueViolation", () => "unique" as const),
        Match.tag("ConstraintError", () => "constraint" as const),
        Match.orElse(() => undefined),
      ),
  });

type SettingsQueries = Pick<MissalDrizzle["Service"], "select" | "insert">;

const decodeSettings = decodeStored(Schema.decodeUnknownEffect(AppSettings), "settings.decode");

export const readSettings = (db: SettingsQueries) =>
  db
    .select()
    .from(appSettings)
    .pipe(
      Effect.flatMap((rows) =>
        Option.match(Array.head(rows), {
          onNone: () => Effect.succeed(DEFAULT_SETTINGS),
          onSome: decodeSettings,
        }),
      ),
    );

export const writeSettings = (
  db: SettingsQueries,
  values: Omit<typeof appSettings.$inferInsert, "id">,
) =>
  db
    .insert(appSettings)
    .values(values)
    .onConflictDoUpdate({ target: appSettings.id, set: values })
    .returning()
    .pipe(
      Effect.flatMap((rows) => requireRow(rows, missingWrite("settings.save"))),
      Effect.flatMap(decodeSettings),
    );

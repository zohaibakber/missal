import { Context, Effect, Layer } from "effect";
import { layer as sqliteClientLayer } from "@effect/sql-sqlite-node/SqliteClient";
import { sql } from "drizzle-orm";
import * as SQLiteNodeDrizzle from "drizzle-orm/effect-sqlite-node";
import { migrate } from "drizzle-orm/effect-sqlite-node/migrator";
import { StorageError } from "#/lib/storage-errors";

export class DatabasePath extends Context.Service<DatabasePath, string>()("missal/DatabasePath") {}

export class MigrationsFolder extends Context.Service<MigrationsFolder, string>()(
  "missal/MigrationsFolder",
) {}

export class MissalDrizzle extends Context.Service<
  MissalDrizzle,
  SQLiteNodeDrizzle.EffectSQLiteNodeDatabase
>()("missal/MissalDrizzle") {}

const SqliteClientFromPathLive = Layer.unwrap(
  Effect.map(DatabasePath, (filename) => sqliteClientLayer({ filename })),
);

export const MissalDrizzleLive = Layer.effect(
  MissalDrizzle,
  Effect.gen(function* () {
    const db = yield* SQLiteNodeDrizzle.makeWithDefaults();
    const initFailed = Effect.mapError(
      () =>
        new StorageError({
          message: "Storage operation failed",
          operation: "database.init",
        }),
    );
    yield* db.run(sql`PRAGMA foreign_keys = ON`).pipe(initFailed);
    yield* db.run(sql`PRAGMA synchronous = NORMAL`).pipe(initFailed);
    const enabled = yield* db
      .get<{ foreign_keys: number }>(sql`PRAGMA foreign_keys`)
      .pipe(initFailed);

    if (enabled?.foreign_keys !== 1) {
      return yield* new StorageError({
        message: "SQLite foreign keys could not be enabled",
        operation: "database.init",
      });
    }

    yield* migrate(db, { migrationsFolder: yield* MigrationsFolder }).pipe(
      Effect.catchTags({
        MigratorInitError: () =>
          Effect.fail(
            new StorageError({
              message: "Database migrations are missing",
              operation: "database.migrate",
            }),
          ),
        EffectDrizzleQueryError: () =>
          Effect.fail(
            new StorageError({
              message: "Storage operation failed",
              operation: "database.migrate",
            }),
          ),
        SqlError: () =>
          Effect.fail(
            new StorageError({
              message: "Storage operation failed",
              operation: "database.migrate",
            }),
          ),
      }),
    );
    return db;
  }),
).pipe(Layer.provide(SqliteClientFromPathLive));

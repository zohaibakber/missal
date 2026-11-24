import * as D1Client from "@effect/sql-d1/D1Client";
import type { Query } from "drizzle-orm";
import * as D1Drizzle from "drizzle-orm/effect-d1";
import type { EffectSQLiteD1QueryEffectHKT } from "drizzle-orm/effect-d1/session";
import type { SQLiteEffectDatabase } from "drizzle-orm/sqlite-core/effect";
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as SqlClient from "effect/sql/SqlClient";
import type { SqlError } from "effect/sql/SqlError";

export class Db extends Context.Service<
  Db,
  SQLiteEffectDatabase<EffectSQLiteD1QueryEffectHKT, unknown>
>()("Db") {
  static readonly layerD1 = Layer.effect(Db, D1Drizzle.makeWithDefaults({}));
}

export interface Compiled {
  readonly toSQL: () => Query;
}

export class Atomic extends Context.Service<
  Atomic,
  { readonly run: (queries: ReadonlyArray<Compiled>) => Effect.Effect<void, SqlError> }
>()("Atomic") {
  static readonly layerD1 = Layer.effect(
    Atomic,
    Effect.map(D1Client.D1Client, (client) =>
      Atomic.of({
        run: (queries) =>
          Effect.asVoid(
            client.batch(
              queries.map((query) => {
                const { sql, params } = query.toSQL();
                return client.unsafe(sql, params);
              }),
            ),
          ),
      }),
    ),
  );

  static readonly layerTransaction = Layer.effect(
    Atomic,
    Effect.map(SqlClient.SqlClient, (sql) =>
      Atomic.of({
        run: (queries) =>
          sql.withTransaction(
            Effect.forEach(
              queries,
              (query) => {
                const compiled = query.toSQL();
                return sql.unsafe(compiled.sql, compiled.params);
              },
              { discard: true },
            ),
          ),
      }),
    ),
  );
}

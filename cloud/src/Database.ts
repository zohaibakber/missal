import * as D1Client from "@effect/sql-d1/D1Client";
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as SqlClient from "effect/sql/SqlClient";
import type { SqlError } from "effect/sql/SqlError";
import type * as Statement from "effect/sql/Statement";
import * as Str from "effect/String";

export const columnNames = {
  transformQueryNames: Str.camelToSnake,
  transformResultNames: Str.snakeToCamel,
} as const;

export class Atomic extends Context.Service<
  Atomic,
  {
    readonly run: (
      statements: ReadonlyArray<Statement.Statement<unknown>>,
    ) => Effect.Effect<void, SqlError>;
  }
>()("Atomic") {
  static readonly layerD1 = Layer.effect(
    Atomic,
    Effect.map(D1Client.D1Client, (client) =>
      Atomic.of({ run: (statements) => Effect.asVoid(client.batch(statements)) }),
    ),
  );

  static readonly layerTransaction = Layer.effect(
    Atomic,
    Effect.map(SqlClient.SqlClient, (sql) =>
      Atomic.of({
        run: (statements) =>
          sql.withTransaction(Effect.forEach(statements, (statement) => statement)),
      }),
    ),
  );
}

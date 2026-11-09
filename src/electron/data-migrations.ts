import { Effect } from "effect";
import * as Migrator from "@effect/sql-sqlite-node/SqliteMigrator";
import { SqlClient } from "effect/sql/SqlClient";
import { FIR_FIELDS, type FirFieldReference } from "#/lib/field";
import { fieldLookupKey } from "#/lib/placeholder";
import { mapReferences, type JsonObject, type JsonValue } from "#/lib/template-bundle";

const firFieldsByName = new Map(
  FIR_FIELDS.map(({ label, reference }) => [fieldLookupKey(label), reference]),
);

const foldPlaceholdersIntoFirFields = Effect.gen(function* () {
  const sql = yield* SqlClient;

  const placeholders = yield* sql<{ id: number; label: string }>`
    SELECT id, label FROM placeholders`;
  const replacements = new Map<number, FirFieldReference>();
  for (const { id, label } of placeholders) {
    const reference = firFieldsByName.get(fieldLookupKey(label));
    if (reference) replacements.set(id, reference);
  }
  if (!replacements.size) return;
  const ids = [...replacements.keys()];

  const values = yield* sql<{ fir_id: number; placeholder_id: number; value: string }>`
    SELECT fir_id, placeholder_id, value FROM fir_placeholder_values
    WHERE ${sql.in("placeholder_id", ids)} AND value <> ''`;
  for (const { fir_id, placeholder_id, value } of values) {
    const reference = replacements.get(placeholder_id);
    if (!reference || reference.index !== undefined) continue;
    const column = sql(reference.property);
    yield* sql`UPDATE fir_records SET ${column} = ${value} WHERE id = ${fir_id} AND ${column} = ''`;
  }

  const replace = (reference: JsonObject): JsonValue => {
    const replacement =
      reference._tag === "CatalogField" && typeof reference.id === "number"
        ? replacements.get(reference.id)
        : undefined;
    return replacement ? { ...replacement } : reference;
  };

  for (const table of ["templates", "fir_documents"]) {
    const rows = yield* sql<{ id: number; document: string; field_references: string }>`
      SELECT id, document, field_references FROM ${sql(table)}`;
    for (const row of rows) {
      const document = JSON.stringify(mapReferences(JSON.parse(row.document), replace));
      const references = JSON.stringify(
        (JSON.parse(row.field_references) as JsonObject[]).map(replace),
      );
      if (document === row.document && references === row.field_references) continue;
      yield* sql`UPDATE ${sql(table)} SET document = ${document}, field_references = ${references} WHERE id = ${row.id}`;
    }
  }

  yield* sql`DELETE FROM placeholders WHERE ${sql.in("id", ids)}`;
});

export const runDataMigrations = Migrator.run({
  loader: Migrator.fromRecord({
    "1_fold_placeholders_into_fir_fields": foldPlaceholdersIntoFirFields,
  }),
  table: "data_migrations",
});

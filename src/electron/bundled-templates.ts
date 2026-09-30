import { readFileSync } from "node:fs";
import path from "node:path";
import { Effect, Match, Schema } from "effect";
import { eq } from "drizzle-orm";
import { MissalDrizzle } from "#/electron/database";
import { bundledTemplates, placeholders, templates } from "#/electron/database-schema";
import {
  decodeStored,
  mapTransaction,
  missingWrite,
  requireRow,
} from "#/electron/repository-helpers";
import {
  BUNDLED_TEMPLATES_INDEX,
  BundledTemplateIndex,
  bundledTemplateFileName,
  mapDocumentFields,
} from "#/lib/bundled-templates";
import { documentWriteColumns, DocumentEnvelope, projectDocument } from "#/lib/document-format";
import { CatalogFieldReference, CustomSource, type FieldSource } from "#/lib/field";
import { PlaceholderId } from "#/lib/ids";
import {
  createDefaultPlaceholders,
  indexPlaceholders,
  Placeholder,
  resolvePlaceholder,
} from "#/lib/placeholder";
import { StorageError } from "#/lib/storage-errors";
import { nowIso } from "#/lib/time";

const defaultIndex = indexPlaceholders(createDefaultPlaceholders());

const sourceKey = (source: FieldSource) =>
  Match.valueTags(source, {
    FirProperty: ({ property, index }) => `fir:${property}:${index ?? ""}`,
    SharedSetting: ({ setting }) => `setting:${setting}`,
    Custom: () => "custom",
  });

const readJson = (file: string) =>
  Effect.try({
    try: () => JSON.parse(readFileSync(file, "utf8")) as unknown,
    catch: () =>
      new StorageError({ message: `Could not read ${file}`, operation: "bundledTemplates.read" }),
  });

/**
 * Installs the templates shipped in `folder` and updates ones the user hasn't edited. A template
 * the user edited or deleted is left alone. A missing folder installs nothing.
 */
export const syncBundledTemplates = Effect.fn("syncBundledTemplates")(function* (folder: string) {
  const indexFile = path.join(folder, BUNDLED_TEMPLATES_INDEX);
  const entries = yield* readJson(indexFile).pipe(
    Effect.flatMap(Schema.decodeUnknownEffect(BundledTemplateIndex)),
    Effect.catch(() => Effect.succeed([])),
  );
  if (entries.length === 0) return;

  const db = yield* MissalDrizzle;
  const installed = new Map(
    (yield* db.select().from(bundledTemplates)).map((row) => [row.name, row]),
  );
  const pending = entries.filter(
    (entry) => installed.get(entry.name)?.sourceHash !== entry.sourceHash,
  );
  if (pending.length === 0) return;

  const documents = yield* Effect.forEach(pending, (entry) =>
    readJson(path.join(folder, bundledTemplateFileName(entry.name))).pipe(
      Effect.flatMap(Schema.decodeUnknownEffect(DocumentEnvelope)),
      Effect.map((document) => ({ entry, document })),
    ),
  );

  yield* db
    .transaction(
      Effect.fnUntraced(function* (tx) {
        const catalog = yield* decodeStored(
          Schema.decodeUnknownEffect(Schema.Array(Placeholder)),
          "bundledTemplates.catalog",
        )(yield* tx.select().from(placeholders));
        const fields = [...catalog];

        // Packs name their fields. A default field is matched by what it is bound to, so it still
        // links after the user renames it; any other name is matched by name or created.
        const fieldId = Effect.fnUntraced(function* (name: string) {
          const seeded = resolvePlaceholder(name, defaultIndex);
          const bound =
            seeded && seeded.source._tag !== "Custom"
              ? fields.find((field) => sourceKey(field.source) === sourceKey(seeded.source))
              : undefined;
          const found = bound ?? resolvePlaceholder(name, indexPlaceholders(fields));
          if (found) return found.id;
          const rows = yield* tx
            .insert(placeholders)
            .values({ label: name, source: CustomSource.make({}) })
            .returning({ id: placeholders.id });
          const row = yield* requireRow(rows, missingWrite("bundledTemplates.field"));
          const created = new Placeholder({
            id: PlaceholderId.make(row.id),
            label: name,
            source: CustomSource.make({}),
          });
          fields.push(created);
          return created.id;
        });

        for (const { entry, document } of documents) {
          const ids = new Map<string, PlaceholderId>();
          for (const reference of projectDocument(document).fieldReferences) {
            if (reference._tag === "UnresolvedToken") {
              ids.set(reference.text, yield* fieldId(reference.text));
            }
          }
          const linked = mapDocumentFields(document, (reference) => {
            const id = reference._tag === "UnresolvedToken" ? ids.get(reference.text) : undefined;
            return id === undefined ? reference : CatalogFieldReference.make({ id });
          });

          const timestamp = yield* nowIso;
          const previous = installed.get(entry.name);
          if (!previous) {
            const rows = yield* tx
              .insert(templates)
              .values({
                name: entry.name,
                revision: 1,
                createdAt: timestamp,
                updatedAt: timestamp,
                ...documentWriteColumns(linked),
              })
              .returning({ id: templates.id });
            const template = yield* requireRow(rows, missingWrite("bundledTemplates.template"));
            yield* tx.insert(bundledTemplates).values({
              name: entry.name,
              sourceHash: entry.sourceHash,
              templateId: template.id,
              templateRevision: 1,
            });
            continue;
          }

          const [current] =
            previous.templateId === null
              ? []
              : yield* tx
                  .select({ revision: templates.revision })
                  .from(templates)
                  .where(eq(templates.id, previous.templateId));
          let templateRevision = previous.templateRevision;
          if (previous.templateId !== null && current?.revision === previous.templateRevision) {
            templateRevision = current.revision + 1;
            yield* tx
              .update(templates)
              .set({
                revision: templateRevision,
                updatedAt: timestamp,
                ...documentWriteColumns(linked),
              })
              .where(eq(templates.id, previous.templateId));
          }
          yield* tx
            .update(bundledTemplates)
            .set({ sourceHash: entry.sourceHash, templateRevision })
            .where(eq(bundledTemplates.name, entry.name));
        }
      }),
    )
    .pipe((effect) => mapTransaction("bundledTemplates.sync", effect));
});

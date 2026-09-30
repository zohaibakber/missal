import { readFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { gunzip } from "node:zlib";
import { Context, Effect, HashMap, Layer, Match, Ref, Schema } from "effect";
import { eq } from "drizzle-orm";
import { MissalDrizzle } from "#/electron/database";
import {
  bundledTemplates,
  placeholders,
  templatePackState,
  templates,
} from "#/electron/database-schema";
import {
  decodeStored,
  mapTransaction,
  missingWrite,
  requireRow,
} from "#/electron/repository-helpers";
import {
  TEMPLATE_PACK_BATCH_BYTES,
  TEMPLATE_PACK_BATCH_TEMPLATES,
  TEMPLATE_PACK_MANIFEST,
  TemplatePackEntry,
  TemplatePackIdle,
  TemplatePackManifest,
  TemplatePackPartialFailure,
  TemplatePackReady,
  TemplatePackSynchronizing,
  mapDocumentFields,
  templatePackBodyPath,
  type TemplatePackStatus,
} from "#/lib/bundled-templates";
import { documentWriteColumns, DocumentEnvelope, projectDocument } from "#/lib/document-format";
import { CatalogFieldReference, CustomSource, type FieldSource } from "#/lib/field";
import { PlaceholderId } from "#/lib/ids";
import {
  createDefaultPlaceholders,
  indexPlaceholders,
  placeholderLookupKey,
  Placeholder,
  resolvePlaceholder,
} from "#/lib/placeholder";
import { StorageError } from "#/lib/storage-errors";
import { nowIso } from "#/lib/time";

const defaultIndex = indexPlaceholders(createDefaultPlaceholders());
const unzip = promisify(gunzip);
const PACK_STATE_ID = "current";

const sourceKey = (source: FieldSource) =>
  Match.valueTags(source, {
    FirProperty: ({ property, index }) => `fir:${property}:${index ?? ""}`,
    SharedSetting: ({ setting }) => `setting:${setting}`,
    Custom: () => "custom",
  });

export class TemplatePackStatusStore extends Context.Service<
  TemplatePackStatusStore,
  Ref.Ref<TemplatePackStatus>
>()("missal/TemplatePackStatusStore") {}

export const TemplatePackStatusStoreLive = Layer.effect(
  TemplatePackStatusStore,
  Ref.make<TemplatePackStatus>(TemplatePackIdle.make({})),
);

const readPackJson = <A>(
  folder: string,
  file: string,
  decode: (input: unknown) => Effect.Effect<A, unknown>,
) =>
  Effect.tryPromise(async () => {
    const bytes = await readFile(path.join(folder, file));
    const json = file.endsWith(".gz")
      ? await unzip(bytes, { maxOutputLength: TEMPLATE_PACK_BATCH_BYTES })
      : bytes;
    return JSON.parse(json.toString("utf8")) as unknown;
  }).pipe(
    Effect.flatMap(decode),
    Effect.mapError(
      () =>
        new StorageError({ message: `Could not read ${file}`, operation: "bundledTemplates.read" }),
    ),
  );

const readBody = (folder: string, entry: TemplatePackEntry) =>
  entry.bytes > TEMPLATE_PACK_BATCH_BYTES
    ? Effect.fail(
        new StorageError({
          message: `${entry.name} is too large to install`,
          operation: "bundledTemplates.read",
        }),
      )
    : readPackJson(
        folder,
        templatePackBodyPath(entry.bodyHash),
        Schema.decodeUnknownEffect(DocumentEnvelope),
      );

function* batches(entries: readonly TemplatePackEntry[]) {
  let batch: TemplatePackEntry[] = [];
  let bytes = 0;
  for (const entry of entries) {
    if (
      batch.length === TEMPLATE_PACK_BATCH_TEMPLATES ||
      (batch.length > 0 && bytes + entry.bytes > TEMPLATE_PACK_BATCH_BYTES)
    ) {
      yield batch;
      batch = [];
      bytes = 0;
    }
    batch.push(entry);
    bytes += entry.bytes;
  }
  if (batch.length > 0) yield batch;
}

/**
 * Installs the templates shipped in `folder` and updates ones the user hasn't edited. A template
 * the user edited or deleted is left alone. Bodies are decoded a batch at a time, outside the
 * transaction that applies them, so user saves run between batches.
 */
export const syncBundledTemplates = Effect.fn("syncBundledTemplates")(function* (folder: string) {
  const status = yield* TemplatePackStatusStore;
  const db = yield* MissalDrizzle;
  const pack = yield* readPackJson(
    folder,
    TEMPLATE_PACK_MANIFEST,
    Schema.decodeUnknownEffect(TemplatePackManifest),
  );

  const [completed] = yield* db.select().from(templatePackState);
  if (completed?.packHash === pack.packHash) {
    yield* Ref.set(status, TemplatePackReady.make({}));
    return;
  }

  const installed = new Map(
    (yield* db
      .select({
        name: bundledTemplates.name,
        bodyHash: bundledTemplates.bodyHash,
        templateId: bundledTemplates.templateId,
        templateRevision: bundledTemplates.templateRevision,
        revision: templates.revision,
      })
      .from(bundledTemplates)
      .leftJoin(templates, eq(templates.id, bundledTemplates.templateId))).map((row) => [
      row.name,
      row,
    ]),
  );
  const pending: TemplatePackEntry[] = [];
  const kept: TemplatePackEntry[] = [];
  for (const entry of pack.entries) {
    const row = installed.get(entry.name);
    if (row?.bodyHash === entry.bodyHash) continue;
    if (row && row.revision !== row.templateRevision) kept.push(entry);
    else pending.push(entry);
  }

  // An edited or deleted template only records that this body was seen.
  if (kept.length > 0) {
    yield* db
      .transaction((tx) =>
        Effect.forEach(
          kept,
          (entry) =>
            tx
              .update(bundledTemplates)
              .set({ bodyHash: entry.bodyHash })
              .where(eq(bundledTemplates.name, entry.name)),
          { discard: true },
        ),
      )
      .pipe((effect) => mapTransaction("bundledTemplates.sync", effect));
  }

  const total = pack.entries.length;
  let done = total - pending.length;
  const failures: string[] = [];
  yield* Ref.set(status, TemplatePackSynchronizing.make({ done, total }));

  for (const batch of batches(pending)) {
    const ready: { readonly entry: TemplatePackEntry; readonly document: DocumentEnvelope }[] = [];
    for (const entry of batch) {
      const document = yield* readBody(folder, entry).pipe(
        Effect.tapError((error) => Effect.sync(() => failures.push(error.message))),
        Effect.option,
      );
      if (document._tag === "Some") ready.push({ entry, document: document.value });
    }

    yield* db
      .transaction(
        Effect.fnUntraced(function* (tx) {
          const catalog = yield* decodeStored(
            Schema.decodeUnknownEffect(Schema.Array(Placeholder)),
            "bundledTemplates.catalog",
          )(yield* tx.select().from(placeholders));
          const emptyById = indexPlaceholders([]).byId;
          let byLabel = indexPlaceholders(catalog).byLabel;
          const bySource = new Map<string, Placeholder>();
          for (const field of catalog) {
            if (field.source._tag !== "Custom") bySource.set(sourceKey(field.source), field);
          }

          // Packs name their fields. A default field is matched by what it is bound to, so it
          // still links after the user renames it; any other name is matched by name or created.
          const fieldId = Effect.fnUntraced(function* (name: string) {
            const seeded = resolvePlaceholder(name, defaultIndex);
            const bound =
              seeded && seeded.source._tag !== "Custom"
                ? bySource.get(sourceKey(seeded.source))
                : undefined;
            const found = bound ?? resolvePlaceholder(name, { byId: emptyById, byLabel });
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
            byLabel = HashMap.set(byLabel, placeholderLookupKey(created.label), created);
            return created.id;
          });

          const link = Effect.fnUntraced(function* (document: DocumentEnvelope) {
            const ids = new Map<string, PlaceholderId>();
            for (const reference of projectDocument(document).fieldReferences) {
              if (reference._tag === "UnresolvedToken") {
                ids.set(reference.text, yield* fieldId(reference.text));
              }
            }
            return mapDocumentFields(document, (reference) => {
              const id = reference._tag === "UnresolvedToken" ? ids.get(reference.text) : undefined;
              return id === undefined ? reference : CatalogFieldReference.make({ id });
            });
          });

          for (const { entry, document } of ready) {
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
                  ...documentWriteColumns(yield* link(document)),
                })
                .returning({ id: templates.id });
              const template = yield* requireRow(rows, missingWrite("bundledTemplates.template"));
              yield* tx.insert(bundledTemplates).values({
                name: entry.name,
                bodyHash: entry.bodyHash,
                templateId: template.id,
                templateRevision: 1,
              });
              continue;
            }

            // The user may have edited or deleted the template while this batch was decoded.
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
                  ...documentWriteColumns(yield* link(document)),
                })
                .where(eq(templates.id, previous.templateId));
            }
            yield* tx
              .update(bundledTemplates)
              .set({ bodyHash: entry.bodyHash, templateRevision })
              .where(eq(bundledTemplates.name, entry.name));
          }
        }),
      )
      .pipe((effect) => mapTransaction("bundledTemplates.sync", effect));

    done += batch.length;
    yield* Ref.set(status, TemplatePackSynchronizing.make({ done, total }));
    yield* Effect.yieldNow;
  }

  if (failures.length > 0) {
    yield* Ref.set(
      status,
      TemplatePackPartialFailure.make({
        message:
          failures.length === 1
            ? (failures[0] ?? "")
            : `${failures.length} bundled templates could not be installed`,
      }),
    );
    return;
  }

  yield* db
    .insert(templatePackState)
    .values({ id: PACK_STATE_ID, packHash: pack.packHash })
    .onConflictDoUpdate({ target: templatePackState.id, set: { packHash: pack.packHash } });
  yield* Ref.set(status, TemplatePackReady.make({}));
});

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { gunzipSync } from "node:zlib";
import { Context, Effect, HashMap, Layer, Match, Option, Ref, Schema } from "effect";
import { and, eq } from "drizzle-orm";
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
  TEMPLATE_PACK_MAX_DOCUMENT_BYTES,
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

const defaultFields = createDefaultPlaceholders();
const defaultIndex = indexPlaceholders(defaultFields);

const sourceKey = (source: FieldSource) =>
  Match.valueTags(source, {
    FirProperty: ({ property, index }) => `fir:${property}:${index ?? ""}`,
    SharedSetting: ({ setting }) => `setting:${setting}`,
    Custom: () => "custom",
  });

const readFailure = (file: string) =>
  new StorageError({ message: `Could not read ${file}`, operation: "bundledTemplates.read" });

const readFileBytes = (file: string) =>
  Effect.tryPromise({
    try: () => readFile(file),
    catch: () => readFailure(file),
  });

export class BundledPackFiles extends Context.Service<
  BundledPackFiles,
  {
    readonly readManifest: (folder: string) => Effect.Effect<Uint8Array, StorageError>;
    readonly readBody: (
      folder: string,
      bodyHash: string,
    ) => Effect.Effect<Uint8Array, StorageError>;
  }
>()("missal/BundledPackFiles") {}

export const BundledPackFilesLive = Layer.sync(BundledPackFiles, () =>
  BundledPackFiles.of({
    readManifest: (folder) => readFileBytes(path.join(folder, TEMPLATE_PACK_MANIFEST)),
    readBody: (folder, bodyHash) =>
      readFileBytes(path.join(folder, templatePackBodyPath(bodyHash))),
  }),
);

export class TemplatePackStatusStore extends Context.Service<
  TemplatePackStatusStore,
  Ref.Ref<TemplatePackStatus>
>()("missal/TemplatePackStatusStore") {}

export const TemplatePackStatusStoreLive = Layer.effect(
  TemplatePackStatusStore,
  Ref.make<TemplatePackStatus>(TemplatePackIdle.make({})),
);

/**
 * Key order is the pack hash. scripts/template-pack/format.mjs must stringify
 * the same fields in the same order.
 */
export function templatePackHash(manifest: {
  readonly formatVersion: number;
  readonly schemaVersion: number;
  readonly converterVersion: number;
  readonly entries: readonly {
    readonly name: string;
    readonly displayName: string;
    readonly bodyHash: string;
    readonly sourceHash: string;
    readonly decodedBytes: number;
    readonly fieldNames: readonly string[];
  }[];
}) {
  return createHash("sha256")
    .update(
      JSON.stringify({
        converterVersion: manifest.converterVersion,
        entries: manifest.entries.map((entry) => ({
          bodyHash: entry.bodyHash,
          decodedBytes: entry.decodedBytes,
          displayName: entry.displayName,
          fieldNames: [...entry.fieldNames],
          name: entry.name,
          sourceHash: entry.sourceHash,
        })),
        formatVersion: manifest.formatVersion,
        schemaVersion: manifest.schemaVersion,
      }),
    )
    .digest("hex");
}

type Eligible = {
  readonly entry: TemplatePackEntry;
  readonly converterVersion: number;
};

type Failure = { readonly name: string; readonly message: string };

const packStateId = "current";

export const syncBundledTemplates = Effect.fn("syncBundledTemplates")(function* (
  folder: string,
  beforeBatch: (batchIndex: number) => Effect.Effect<void>,
) {
  const status = yield* TemplatePackStatusStore;
  const files = yield* BundledPackFiles;
  const db = yield* MissalDrizzle;
  const fail = (message: string) =>
    Ref.set(status, TemplatePackPartialFailure.make({ message })).pipe(
      Effect.andThen(Effect.logWarning(message)),
    );

  const loaded = yield* files.readManifest(folder).pipe(
    Effect.flatMap((bytes) =>
      Effect.try({
        try: () => JSON.parse(new TextDecoder().decode(bytes)) as unknown,
        catch: () => readFailure(TEMPLATE_PACK_MANIFEST),
      }),
    ),
    Effect.flatMap(Schema.decodeUnknownEffect(TemplatePackManifest)),
    Effect.mapError(() => readFailure(TEMPLATE_PACK_MANIFEST)),
    Effect.option,
  );
  if (Option.isNone(loaded)) {
    yield* fail("Template pack could not be read");
    return;
  }
  const pack = loaded.value;
  if (templatePackHash(pack) !== pack.packHash) {
    yield* fail("Template pack hash does not match");
    return;
  }
  const identities = new Set<string>();
  for (const entry of pack.entries) {
    if (identities.has(entry.name)) {
      yield* fail("Template pack has a duplicate identity");
      return;
    }
    identities.add(entry.name);
  }

  const [stored] = yield* db
    .select({ packHash: templatePackState.packHash })
    .from(templatePackState)
    .where(eq(templatePackState.id, packStateId));
  if (stored?.packHash === pack.packHash) {
    yield* Ref.set(status, TemplatePackReady.make({}));
    return;
  }

  const installed = yield* db
    .select({
      bodyHash: bundledTemplates.bodyHash,
      converterVersion: bundledTemplates.converterVersion,
      currentRevision: templates.revision,
      name: bundledTemplates.name,
      sourceHash: bundledTemplates.sourceHash,
      templateId: bundledTemplates.templateId,
      templateRevision: bundledTemplates.templateRevision,
    })
    .from(bundledTemplates)
    .leftJoin(templates, eq(templates.id, bundledTemplates.templateId));
  const installedByName = new Map(installed.map((row) => [row.name, row]));

  const metadata: TemplatePackEntry[] = [];
  const queue: Eligible[] = [];
  let done = 0;
  for (const entry of pack.entries) {
    const row = installedByName.get(entry.name);
    const observed =
      row !== undefined &&
      row.bodyHash === entry.bodyHash &&
      row.sourceHash === entry.sourceHash &&
      row.converterVersion === pack.converterVersion;
    if (observed) {
      done += 1;
      continue;
    }
    if (!row) {
      queue.push({ converterVersion: pack.converterVersion, entry });
      continue;
    }
    const deleted = row.templateId === null || row.currentRevision === null;
    const edited = !deleted && row.currentRevision !== row.templateRevision;
    if (deleted || edited) metadata.push(entry);
    else queue.push({ converterVersion: pack.converterVersion, entry });
  }

  const total = pack.entries.length;
  const failures: Failure[] = [];
  const progress = () => Ref.set(status, TemplatePackSynchronizing.make({ done, total }));
  yield* progress();

  if (metadata.length > 0) {
    yield* db
      .transaction(
        Effect.fnUntraced(function* (tx) {
          yield* Effect.forEach(
            metadata,
            (entry) =>
              tx
                .update(bundledTemplates)
                .set({
                  bodyHash: entry.bodyHash,
                  converterVersion: pack.converterVersion,
                  sourceHash: entry.sourceHash,
                })
                .where(eq(bundledTemplates.name, entry.name)),
            { concurrency: 1, discard: true },
          );
        }),
      )
      .pipe((effect) => mapTransaction("bundledTemplates.sync", effect));
    done += metadata.length;
    yield* progress();
    yield* Effect.yieldNow;
  }

  let batchIndex = 0;
  while (queue.length > 0) {
    const batch: Eligible[] = [];
    let bytes = 0;
    while (queue.length > 0 && batch.length < TEMPLATE_PACK_BATCH_TEMPLATES) {
      const next = queue[0];
      if (!next) break;
      if (
        next.entry.decodedBytes > TEMPLATE_PACK_MAX_DOCUMENT_BYTES ||
        next.entry.decodedBytes > TEMPLATE_PACK_BATCH_BYTES
      ) {
        queue.shift();
        failures.push({
          message: `Template ${next.entry.name} exceeds the pack size limit`,
          name: next.entry.name,
        });
        done += 1;
        continue;
      }
      if (batch.length > 0 && bytes + next.entry.decodedBytes > TEMPLATE_PACK_BATCH_BYTES) break;
      queue.shift();
      bytes += next.entry.decodedBytes;
      batch.push(next);
    }
    if (batch.length === 0) {
      yield* progress();
      continue;
    }

    yield* beforeBatch(batchIndex);
    const decoded = yield* Effect.forEach(
      batch,
      (item) =>
        decodeBody(files, folder, item).pipe(
          Effect.map((document) => ({ _tag: "Ready" as const, document, item })),
          Effect.catch((error) =>
            Effect.succeed({
              _tag: "Failed" as const,
              message: error.message,
              name: item.entry.name,
            }),
          ),
        ),
      { concurrency: 1 },
    );
    const ready = decoded.flatMap((item) => (item._tag === "Ready" ? [item] : []));
    for (const item of decoded) {
      if (item._tag === "Failed") failures.push({ message: item.message, name: item.name });
    }
    if (ready.length > 0) {
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
                const id =
                  reference._tag === "UnresolvedToken" ? ids.get(reference.text) : undefined;
                return id === undefined ? reference : CatalogFieldReference.make({ id });
              });
            });

            yield* Effect.forEach(
              ready,
              (item) =>
                Effect.gen(function* () {
                  const { entry } = item.item;
                  const observe = (templateRevision?: number) =>
                    tx
                      .update(bundledTemplates)
                      .set({
                        bodyHash: entry.bodyHash,
                        converterVersion: item.item.converterVersion,
                        sourceHash: entry.sourceHash,
                        ...(templateRevision === undefined ? {} : { templateRevision }),
                      })
                      .where(eq(bundledTemplates.name, entry.name));
                  const [row] = yield* tx
                    .select()
                    .from(bundledTemplates)
                    .where(eq(bundledTemplates.name, entry.name));
                  if (row?.templateId != null) {
                    const [current] = yield* tx
                      .select({ revision: templates.revision })
                      .from(templates)
                      .where(eq(templates.id, row.templateId));
                    // A revision mismatch means the user edited after this body was last
                    // applied. Leave template_revision at that applied revision so a later
                    // pack still treats the template as edited.
                    if (!current || current.revision !== row.templateRevision) {
                      yield* observe();
                      return;
                    }
                    const linked = yield* link(item.document);
                    const timestamp = yield* nowIso;
                    const updated = yield* tx
                      .update(templates)
                      .set({
                        name: entry.displayName,
                        revision: current.revision + 1,
                        updatedAt: timestamp,
                        ...documentWriteColumns(linked),
                      })
                      .where(
                        and(
                          eq(templates.id, row.templateId),
                          eq(templates.revision, current.revision),
                        ),
                      )
                      .returning({ revision: templates.revision });
                    if (updated.length === 0) {
                      yield* observe();
                      return;
                    }
                    const applied = updated[0];
                    if (!applied) return yield* missingWrite("bundledTemplates.template");
                    yield* observe(applied.revision);
                    return;
                  }
                  if (row) {
                    yield* observe();
                    return;
                  }
                  const linked = yield* link(item.document);
                  const timestamp = yield* nowIso;
                  const inserted = yield* tx
                    .insert(templates)
                    .values({
                      createdAt: timestamp,
                      name: entry.displayName,
                      revision: 1,
                      updatedAt: timestamp,
                      ...documentWriteColumns(linked),
                    })
                    .returning({ id: templates.id });
                  const template = yield* requireRow(
                    inserted,
                    missingWrite("bundledTemplates.template"),
                  );
                  yield* tx.insert(bundledTemplates).values({
                    bodyHash: entry.bodyHash,
                    converterVersion: item.item.converterVersion,
                    name: entry.name,
                    sourceHash: entry.sourceHash,
                    templateId: template.id,
                    templateRevision: 1,
                  });
                }),
              { concurrency: 1, discard: true },
            );
          }),
        )
        .pipe((effect) => mapTransaction("bundledTemplates.sync", effect));
    }
    done += batch.length;
    batchIndex += 1;
    yield* progress();
    yield* Effect.yieldNow;
  }

  if (failures.length > 0) {
    const message =
      failures.length === 1
        ? (failures[0]?.message ?? "A template could not be installed")
        : `${failures.length} templates could not be installed`;
    yield* fail(message);
    return;
  }

  yield* db
    .insert(templatePackState)
    .values({ id: packStateId, packHash: pack.packHash })
    .onConflictDoUpdate({
      set: { packHash: pack.packHash },
      target: templatePackState.id,
    });
  yield* Ref.set(status, TemplatePackReady.make({}));
});

const decodeBody = Effect.fnUntraced(function* (
  files: BundledPackFiles["Service"],
  folder: string,
  item: Eligible,
) {
  const compressed = yield* files.readBody(folder, item.entry.bodyHash);
  const json = yield* Effect.try({
    try: () => gunzipSync(compressed),
    catch: () =>
      new StorageError({
        message: `Could not read template ${item.entry.name}`,
        operation: "bundledTemplates.read",
      }),
  });
  if (
    json.byteLength > TEMPLATE_PACK_MAX_DOCUMENT_BYTES ||
    json.byteLength !== item.entry.decodedBytes ||
    createHash("sha256").update(json).digest("hex") !== item.entry.bodyHash
  ) {
    return yield* new StorageError({
      message: `Could not read template ${item.entry.name}`,
      operation: "bundledTemplates.read",
    });
  }
  const parsed = yield* Effect.try({
    try: () => JSON.parse(new TextDecoder().decode(json)) as unknown,
    catch: () =>
      new StorageError({
        message: `Could not read template ${item.entry.name}`,
        operation: "bundledTemplates.read",
      }),
  });
  return yield* Schema.decodeUnknownEffect(DocumentEnvelope)(parsed).pipe(
    Effect.mapError(
      () =>
        new StorageError({
          message: `Could not read template ${item.entry.name}`,
          operation: "bundledTemplates.read",
        }),
    ),
  );
});

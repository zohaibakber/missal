import { globalPlaceholderOperations } from "#/electron/global-placeholders";
import { Effect, HashMap, Layer, Option, Schema } from "effect";
import { and, asc, count, desc, eq, inArray, like, or } from "drizzle-orm";
import {
  appSettings,
  firDocumentRecordColumns,
  firDocumentSummaryColumns,
  firDocuments,
  firPlaceholderValues,
  firRecords,
  firSummaryColumns,
  placeholders,
  templateRecordColumns,
  templateSummaryColumns,
  templates,
} from "#/electron/database-schema";
import { MissalDrizzle, MissalDrizzleLive } from "#/electron/database";
import {
  constraintKind,
  decodeStored,
  failQuery,
  isDrizzleQueryError,
  likeNeedle,
  mapQuery,
  mapTransaction,
  missingWrite,
  notFound,
  recoverUnique,
  requireRevisionMatch,
  requireRow,
  uniqueConflict,
} from "#/electron/repository-helpers";
import { documentWriteColumns, DocumentEnvelope } from "#/lib/document-format";
import { CustomSource } from "#/lib/field";
import { FirRecord, FirSummary } from "#/lib/fir";
import {
  FirDocumentRecord,
  FirDocumentSaveAck,
  FirDocumentSummary,
  FirValueContext,
} from "#/lib/fir-document";
import { DocumentRevision, type FirId, type PlaceholderId, TemplateId } from "#/lib/ids";
import { Placeholder, PlaceholderCreateInput, PlaceholderUpdateInput } from "#/lib/placeholder";
import { AppSettings } from "#/lib/settings";
import { EntityInUse, type EntityNotFound, type StorageError } from "#/lib/storage-errors";
import type { EffectDrizzleQueryError } from "drizzle-orm/effect-core/errors";
import {
  FirPlaceholderValue,
  TemplateRecord,
  TemplateSaveAck,
  TemplateSummary,
} from "#/lib/templates";
import { nowIso } from "#/lib/time";
import {
  FirDocumentRepository,
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  SettingsRepository,
  TemplateRepository,
} from "#/repositories/index";

const nextRevision = (revision: DocumentRevision) => DocumentRevision.make(revision + 1);

// Decoders are built once; each call site used to rebuild its parser per query.
const decodePlaceholders = decodeStored(
  Schema.decodeUnknownEffect(Schema.Array(Placeholder)),
  "placeholder.decode",
);
const decodePlaceholder = decodeStored(
  Schema.decodeUnknownEffect(Placeholder),
  "placeholder.decode",
);
const decodeTemplateSummaries = decodeStored(
  Schema.decodeUnknownEffect(Schema.Array(TemplateSummary)),
  "template.summary.decode",
);
const decodeTemplateSummary = decodeStored(
  Schema.decodeUnknownEffect(TemplateSummary),
  "template.summary.decode",
);
const decodeTemplateRecord = decodeStored(
  Schema.decodeUnknownEffect(TemplateRecord),
  "template.decode",
);
const decodeTemplateAck = decodeStored(
  Schema.decodeUnknownEffect(TemplateSaveAck),
  "template.ack.decode",
);
const decodeFirSummaries = decodeStored(
  Schema.decodeUnknownEffect(Schema.Array(FirSummary)),
  "fir.decode",
);
const decodeFir = decodeStored(Schema.decodeUnknownEffect(FirRecord), "fir.decode");
const decodeFirDocumentSummaries = decodeStored(
  Schema.decodeUnknownEffect(Schema.Array(FirDocumentSummary)),
  "firDocument.summary.decode",
);
const decodeFirDocumentRecord = decodeStored(
  Schema.decodeUnknownEffect(FirDocumentRecord),
  "firDocument.decode",
);
const decodeFirDocumentRecords = decodeStored(
  Schema.decodeUnknownEffect(Schema.Array(FirDocumentRecord)),
  "firDocument.decode",
);
const decodeFirDocumentAck = decodeStored(
  Schema.decodeUnknownEffect(FirDocumentSaveAck),
  "firDocument.ack.decode",
);
const decodeEnvelope = decodeStored(
  Schema.decodeUnknownEffect(DocumentEnvelope),
  "template.document.decode",
);
const decodeFirPlaceholderValues = decodeStored(
  Schema.decodeUnknownEffect(Schema.Array(FirPlaceholderValue)),
  "firPlaceholderValue.decode",
);
const decodeFirPlaceholderValue = decodeStored(
  Schema.decodeUnknownEffect(FirPlaceholderValue),
  "firPlaceholderValue.decode",
);
const decodeSettings = decodeStored(Schema.decodeUnknownEffect(AppSettings), "settings.decode");

const DrizzlePlaceholderRepositoryLive = Layer.effect(
  PlaceholderRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    const list = Effect.fn("PlaceholderRepository.list")(
      function* () {
        const rows = yield* db.select().from(placeholders).orderBy(placeholders.id);
        return yield* decodePlaceholders(rows);
      },
      (effect) => mapQuery("placeholder.list", effect),
    )();

    const create = Effect.fn("PlaceholderRepository.create")(
      function* (input: PlaceholderCreateInput) {
        const rows = yield* db
          .insert(placeholders)
          .values({ label: input.label, source: CustomSource.make({}) })
          .returning()
          .pipe(
            Effect.catchIf(
              isDrizzleQueryError,
              recoverUnique("placeholder.create", "placeholder", "name", input.label),
            ),
          );
        const row = yield* requireRow(rows, missingWrite("placeholder.create"));
        return yield* decodePlaceholder(row);
      },
      (effect) => mapQuery("placeholder.create", effect),
    );

    const update = Effect.fn("PlaceholderRepository.update")(
      function* (input: PlaceholderUpdateInput) {
        const rows = yield* db
          .update(placeholders)
          .set({ label: input.label })
          .where(eq(placeholders.id, input.id))
          .returning()
          .pipe(
            Effect.catchIf(
              isDrizzleQueryError,
              recoverUnique("placeholder.update", "placeholder", "name", input.label),
            ),
          );
        const row = yield* requireRow(rows, notFound("placeholder", input.id));
        return yield* decodePlaceholder(row);
      },
      (effect) => mapQuery("placeholder.update", effect),
    );

    const remove = Effect.fn("PlaceholderRepository.remove")(
      function* (id) {
        const rows = yield* db.delete(placeholders).where(eq(placeholders.id, id)).returning();
        yield* requireRow(rows, notFound("placeholder", id));
      },
      (effect) => mapQuery("placeholder.remove", effect),
    );

    return PlaceholderRepository.of({
      list,
      create,
      update,
      remove,
      ...globalPlaceholderOperations(db),
    });
  }),
);

const DrizzleTemplateRepositoryLive = Layer.effect(
  TemplateRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    const list = Effect.fn("TemplateRepository.list")(
      function* () {
        const rows = yield* db
          .select(templateSummaryColumns)
          .from(templates)
          .orderBy(desc(templates.updatedAt), desc(templates.id));
        return yield* decodeTemplateSummaries(rows);
      },
      (effect) => mapQuery("template.list", effect),
    )();

    const search = Effect.fn("TemplateRepository.search")(
      function* (query: string) {
        const needle = likeNeedle(query);
        if (needle === undefined) {
          return yield* list;
        }

        const rows = yield* db
          .select(templateSummaryColumns)
          .from(templates)
          .where(or(like(templates.name, needle), like(templates.plainText, needle)))
          .orderBy(desc(templates.updatedAt), desc(templates.id));
        return yield* decodeTemplateSummaries(rows);
      },
      (effect) => mapQuery("template.search", effect),
    );

    const get = Effect.fn("TemplateRepository.get")(
      function* (id) {
        const rows = yield* db
          .select(templateRecordColumns)
          .from(templates)
          .where(eq(templates.id, id));
        const row = yield* requireRow(rows, notFound("template", id));
        return yield* decodeTemplateRecord(row);
      },
      (effect) => mapQuery("template.get", effect),
    );

    const create = Effect.fn("TemplateRepository.create")(
      function* (input) {
        const timestamp = yield* nowIso;
        const rows = yield* db
          .insert(templates)
          .values({
            createdAt: timestamp,
            name: input.name,
            revision: 1,
            updatedAt: timestamp,
            ...documentWriteColumns(input.document),
          })
          .returning(templateSummaryColumns);
        const row = yield* requireRow(rows, missingWrite("template.create"));
        return yield* decodeTemplateSummary(row);
      },
      (effect) => mapQuery("template.create", effect),
    );

    const save = Effect.fn("TemplateRepository.save")(
      function* (input) {
        const timestamp = yield* nowIso;
        const rows = yield* db
          .update(templates)
          .set({
            name: input.name,
            revision: nextRevision(input.expectedRevision),
            updatedAt: timestamp,
            ...documentWriteColumns(input.document),
          })
          .where(and(eq(templates.id, input.id), eq(templates.revision, input.expectedRevision)))
          .returning(templateSummaryColumns);
        const row = yield* requireRevisionMatch(
          rows,
          db.select({ id: templates.id }).from(templates).where(eq(templates.id, input.id)),
          "template",
          input.id,
          input.expectedRevision,
        );
        return yield* decodeTemplateAck({
          fieldCount: row.fieldCount,
          id: row.id,
          previewText: row.previewText,
          revision: row.revision,
          updatedAt: row.updatedAt,
        });
      },
      (effect) => mapQuery("template.save", effect),
    );

    const remove = Effect.fn("TemplateRepository.remove")(
      function* (id) {
        const rows = yield* db
          .delete(templates)
          .where(eq(templates.id, id))
          .returning({
            id: templates.id,
          })
          .pipe(
            Effect.catchIf(isDrizzleQueryError, (error) =>
              Effect.gen(function* () {
                if (constraintKind(error) === "constraint") {
                  const usage = yield* db
                    .select({ total: count() })
                    .from(firDocuments)
                    .where(eq(firDocuments.templateId, id));
                  return yield* new EntityInUse({
                    count: Number(usage[0]?.total ?? 0),
                    entity: "template",
                    id: String(id),
                    usedBy: "firDocument",
                  });
                }

                return yield* failQuery("template.remove")(error);
              }),
            ),
          );
        yield* requireRow(rows, notFound("template", id));
      },
      (effect) => mapQuery("template.remove", effect),
    );

    return TemplateRepository.of({ list, search, get, create, save, remove });
  }),
);

const DrizzleFirRepositoryLive = Layer.effect(
  FirRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    const firColumns = (input: FirRecord | Omit<FirRecord, "id">) => ({
      NIC: input.NIC,
      accused: input.accused,
      arrest_date: input.arrest_date,
      date: input.date,
      fir_no: input.fir_no,
      incident_date: input.incident_date,
      investigation_officer: input.investigation_officer,
      mobile: input.mobile,
      offence: input.offence,
      status: input.status,
      witness: input.witness,
      zimni: input.zimni,
    });

    const list = Effect.fn("FirRepository.list")(
      function* () {
        const rows = yield* db
          .select(firSummaryColumns)
          .from(firRecords)
          .orderBy(desc(firRecords.id));
        return yield* decodeFirSummaries(rows);
      },
      (effect) => mapQuery("fir.list", effect),
    )();

    const get = Effect.fn("FirRepository.get")(
      function* (id) {
        const rows = yield* db.select().from(firRecords).where(eq(firRecords.id, id));
        const row = yield* requireRow(rows, notFound("fir", id));
        return yield* decodeFir(row);
      },
      (effect) => mapQuery("fir.get", effect),
    );

    const create = Effect.fn("FirRepository.create")(
      function* (input) {
        const rows = yield* db.insert(firRecords).values(firColumns(input)).returning();
        const row = yield* requireRow(rows, missingWrite("fir.create"));
        return yield* decodeFir(row);
      },
      (effect) => mapQuery("fir.create", effect),
    );

    const update = Effect.fn("FirRepository.update")(
      function* (input) {
        const rows = yield* db
          .update(firRecords)
          .set(firColumns(input))
          .where(eq(firRecords.id, input.id))
          .returning();
        const row = yield* requireRow(rows, notFound("fir", input.id));
        return yield* decodeFir(row);
      },
      (effect) => mapQuery("fir.update", effect),
    );

    const remove = Effect.fn("FirRepository.remove")(
      function* (id) {
        const rows = yield* db.delete(firRecords).where(eq(firRecords.id, id)).returning({
          id: firRecords.id,
        });
        yield* requireRow(rows, notFound("fir", id));
      },
      (effect) => mapQuery("fir.remove", effect),
    );

    const getValueContext = Effect.fn("FirRepository.getValueContext")(
      function* (id) {
        const firEffect = get(id);
        const catalogEffect = db
          .select()
          .from(placeholders)
          .orderBy(placeholders.id)
          .pipe(Effect.flatMap(decodePlaceholders));
        const overridesEffect = db
          .select()
          .from(firPlaceholderValues)
          .where(eq(firPlaceholderValues.firId, id))
          .pipe(Effect.flatMap(decodeFirPlaceholderValues));
        const settingsEffect = db
          .select()
          .from(appSettings)
          .where(eq(appSettings.id, "default"))
          .pipe(
            Effect.flatMap((rows) => requireRow(rows, missingWrite("settings.get"))),
            Effect.flatMap(decodeSettings),
          );
        // One SQLite connection serves every query, so running these concurrently gains nothing.
        const [fir, catalog, overrides, settings] = yield* Effect.all([
          firEffect,
          catalogEffect,
          overridesEffect,
          settingsEffect,
        ]);

        return new FirValueContext({
          catalog,
          fir,
          overrides,
          sharedSettings: settings.sharedPlaceholders,
        });
      },
      (effect) => mapQuery("fir.valueContext", effect),
    );

    return FirRepository.of({ list, get, create, update, remove, getValueContext });
  }),
);

const DrizzleFirDocumentRepositoryLive = Layer.effect(
  FirDocumentRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    const listForFir = Effect.fn("FirDocumentRepository.listForFir")(
      function* (firId) {
        const rows = yield* db
          .select(firDocumentSummaryColumns)
          .from(firDocuments)
          .where(eq(firDocuments.firId, firId))
          .orderBy(asc(firDocuments.position), asc(firDocuments.id));
        return yield* decodeFirDocumentSummaries(rows);
      },
      (effect) => mapQuery("firDocument.listForFir", effect),
    );

    const get = Effect.fn("FirDocumentRepository.get")(
      function* (id) {
        const rows = yield* db
          .select(firDocumentRecordColumns)
          .from(firDocuments)
          .where(eq(firDocuments.id, id));
        const row = yield* requireRow(rows, notFound("firDocument", id));
        return yield* decodeFirDocumentRecord(row);
      },
      (effect) => mapQuery("firDocument.get", effect),
    );

    const getMany = Effect.fn("FirDocumentRepository.getMany")(
      function* (ids) {
        if (ids.length === 0) {
          return [];
        }

        const rows = yield* db
          .select(firDocumentRecordColumns)
          .from(firDocuments)
          .where(inArray(firDocuments.id, [...ids]));
        const rowsById = new Map(rows.map((row) => [row.id, row]));
        const ordered: (typeof rows)[number][] = [];
        for (const id of ids) {
          const row = rowsById.get(id);
          if (!row) {
            return yield* notFound("firDocument", id);
          }
          ordered.push(row);
        }
        return yield* decodeFirDocumentRecords(ordered);
      },
      (effect) => mapQuery("firDocument.getMany", effect),
    );

    const addTemplates = Effect.fn("FirDocumentRepository.addTemplates")(
      function* (input) {
        return yield* db.transaction(
          Effect.fnUntraced(function* (tx) {
            const firRows = yield* tx
              .select({ id: firRecords.id })
              .from(firRecords)
              .where(eq(firRecords.id, input.firId));
            yield* requireRow(firRows, notFound("fir", input.firId));

            const existing = yield* tx
              .select(firDocumentSummaryColumns)
              .from(firDocuments)
              .where(eq(firDocuments.firId, input.firId));
            const existingByTemplate = HashMap.fromIterable(
              existing.map((row) => [row.templateId, row] as const),
            );

            const requested: TemplateId[] = [];
            const seen = new Set<number>();
            for (const templateId of input.templateIds) {
              if (seen.has(templateId)) {
                continue;
              }
              seen.add(templateId);
              requested.push(templateId);
            }

            const missingTemplateIds = requested.filter((templateId) =>
              Option.isNone(HashMap.get(existingByTemplate, templateId)),
            );

            if (missingTemplateIds.length > 0) {
              const templateRows = yield* tx
                .select()
                .from(templates)
                .where(inArray(templates.id, missingTemplateIds));
              const templatesById = HashMap.fromIterable(
                templateRows.map((row) => [row.id, row] as const),
              );

              for (const templateId of missingTemplateIds) {
                if (Option.isNone(HashMap.get(templatesById, templateId))) {
                  return yield* notFound("template", templateId);
                }
              }

              const maxPosition = existing.reduce(
                (highest, row) => Math.max(highest, row.position),
                -1,
              );
              let position = maxPosition;
              const timestamp = yield* nowIso;

              for (const templateId of requested) {
                if (Option.isSome(HashMap.get(existingByTemplate, templateId))) {
                  continue;
                }

                const template = Option.getOrThrow(HashMap.get(templatesById, templateId));
                const envelope = yield* decodeEnvelope(template.document);
                const write = documentWriteColumns(envelope);
                position += 1;
                yield* tx.insert(firDocuments).values({
                  createdAt: timestamp,
                  firId: input.firId,
                  position,
                  revision: 1,
                  sourceTemplateRevision: template.revision,
                  templateId,
                  title: template.name,
                  updatedAt: timestamp,
                  ...write,
                });
              }
            }

            const rows = yield* tx
              .select(firDocumentSummaryColumns)
              .from(firDocuments)
              .where(eq(firDocuments.firId, input.firId))
              .orderBy(asc(firDocuments.position), asc(firDocuments.id));
            return yield* decodeFirDocumentSummaries(rows);
          }),
        );
      },
      (effect) => mapTransaction("firDocument.addTemplates", effect),
    );

    const save = Effect.fn("FirDocumentRepository.save")(
      function* (input) {
        const timestamp = yield* nowIso;
        const rows = yield* db
          .update(firDocuments)
          .set({
            revision: nextRevision(input.expectedRevision),
            updatedAt: timestamp,
            ...documentWriteColumns(input.document),
          })
          .where(
            and(eq(firDocuments.id, input.id), eq(firDocuments.revision, input.expectedRevision)),
          )
          .returning(firDocumentSummaryColumns);
        const row = yield* requireRevisionMatch(
          rows,
          db
            .select({ id: firDocuments.id })
            .from(firDocuments)
            .where(eq(firDocuments.id, input.id)),
          "firDocument",
          input.id,
          input.expectedRevision,
        );
        return yield* decodeFirDocumentAck({
          fieldCount: row.fieldCount,
          id: row.id,
          previewText: row.previewText,
          revision: row.revision,
          updatedAt: row.updatedAt,
        });
      },
      (effect) => mapQuery("firDocument.save", effect),
    );

    const reorder = Effect.fn("FirDocumentRepository.reorder")(
      function* (input) {
        return yield* db.transaction(
          Effect.fnUntraced(function* (tx) {
            const existing = yield* tx
              .select({ id: firDocuments.id })
              .from(firDocuments)
              .where(eq(firDocuments.firId, input.firId));
            if (existing.length !== input.documentIds.length) {
              return yield* uniqueConflict("firDocument", "order", String(input.firId));
            }

            const existingIds = new Set(existing.map((row) => row.id));
            for (const documentId of input.documentIds) {
              if (!existingIds.has(documentId)) {
                return yield* notFound("firDocument", documentId);
              }
            }

            const timestamp = yield* nowIso;
            for (const [position, documentId] of input.documentIds.entries()) {
              yield* tx
                .update(firDocuments)
                .set({ position, updatedAt: timestamp })
                .where(eq(firDocuments.id, documentId));
            }

            const rows = yield* tx
              .select(firDocumentSummaryColumns)
              .from(firDocuments)
              .where(eq(firDocuments.firId, input.firId))
              .orderBy(asc(firDocuments.position), asc(firDocuments.id));
            return yield* decodeFirDocumentSummaries(rows);
          }),
        );
      },
      (effect) => mapTransaction("firDocument.reorder", effect),
    );

    const remove = Effect.fn("FirDocumentRepository.remove")(
      function* (id) {
        const rows = yield* db.delete(firDocuments).where(eq(firDocuments.id, id)).returning({
          id: firDocuments.id,
        });
        yield* requireRow(rows, notFound("firDocument", id));
      },
      (effect) => mapQuery("firDocument.remove", effect),
    );

    return FirDocumentRepository.of({
      listForFir,
      get,
      getMany,
      addTemplates,
      save,
      reorder,
      remove,
    });
  }),
);

const DrizzleFirPlaceholderValueRepositoryLive = Layer.effect(
  FirPlaceholderValueRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    const missingValueOwner = Effect.fnUntraced(function* (
      firId: FirId,
      placeholderId: PlaceholderId,
    ) {
      const firs = yield* db
        .select({ id: firRecords.id })
        .from(firRecords)
        .where(eq(firRecords.id, firId));
      return yield* firs.length === 0
        ? notFound("fir", firId)
        : notFound("placeholder", placeholderId);
    });

    const recoverMissingOwner =
      (firId: FirId, placeholderId: PlaceholderId) =>
      (
        error: EffectDrizzleQueryError,
      ): Effect.Effect<never, EntityNotFound | StorageError | EffectDrizzleQueryError> =>
        constraintKind(error) === "constraint"
          ? missingValueOwner(firId, placeholderId)
          : failQuery("firPlaceholderValue.upsert")(error);

    return FirPlaceholderValueRepository.of({
      listForFir: Effect.fn("FirPlaceholderValueRepository.listForFir")(
        function* (firId) {
          const rows = yield* db
            .select()
            .from(firPlaceholderValues)
            .where(eq(firPlaceholderValues.firId, firId));
          return yield* decodeFirPlaceholderValues(rows);
        },
        (effect) => mapQuery("firPlaceholderValue.listForFir", effect),
      ),

      upsert: Effect.fn("FirPlaceholderValueRepository.upsert")(
        function* (input) {
          const timestamp = yield* nowIso;
          const rows = yield* db
            .insert(firPlaceholderValues)
            .values({
              firId: input.firId,
              placeholderId: input.placeholderId,
              updatedAt: timestamp,
              value: input.value,
            })
            .onConflictDoUpdate({
              set: {
                updatedAt: timestamp,
                value: input.value,
              },
              target: [firPlaceholderValues.firId, firPlaceholderValues.placeholderId],
            })
            .returning()
            .pipe(
              // Foreign keys already reject a missing FIR or placeholder; only then look up which.
              Effect.catchIf(
                isDrizzleQueryError,
                recoverMissingOwner(input.firId, input.placeholderId),
              ),
            );
          const row = yield* requireRow(rows, missingWrite("firPlaceholderValue.upsert"));
          return yield* decodeFirPlaceholderValue(row);
        },
        (effect) => mapQuery("firPlaceholderValue.upsert", effect),
      ),

      remove: Effect.fn("FirPlaceholderValueRepository.remove")(
        function* (input) {
          const rows = yield* db
            .delete(firPlaceholderValues)
            .where(
              and(
                eq(firPlaceholderValues.firId, input.firId),
                eq(firPlaceholderValues.placeholderId, input.placeholderId),
              ),
            )
            .returning();
          yield* requireRow(
            rows,
            notFound("firPlaceholderValue", `${input.firId}:${input.placeholderId}`),
          );
        },
        (effect) => mapQuery("firPlaceholderValue.remove", effect),
      ),
    });
  }),
);

const DrizzleSettingsRepositoryLive = Layer.effect(
  SettingsRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    return SettingsRepository.of({
      get: Effect.fn("SettingsRepository.get")(
        function* () {
          const rows = yield* db.select().from(appSettings).where(eq(appSettings.id, "default"));
          const row = yield* requireRow(rows, missingWrite("settings.get"));
          return yield* decodeSettings(row);
        },
        (effect) => mapQuery("settings.get", effect),
      )(),

      save: Effect.fn("SettingsRepository.save")(
        function* (sharedPlaceholders) {
          const timestamp = yield* nowIso;
          const rows = yield* db
            .update(appSettings)
            .set({
              sharedPlaceholders: { ...sharedPlaceholders },
              updatedAt: timestamp,
            })
            .where(eq(appSettings.id, "default"))
            .returning();
          const row = yield* requireRow(rows, missingWrite("settings.save"));
          return yield* decodeSettings(row);
        },
        (effect) => mapQuery("settings.save", effect),
      ),

      saveFieldMarkers: Effect.fn("SettingsRepository.saveFieldMarkers")(
        function* (fieldMarkers) {
          const rows = yield* db
            .update(appSettings)
            .set({
              fieldMarkers: { open: fieldMarkers.open, close: fieldMarkers.close },
              updatedAt: yield* nowIso,
            })
            .where(eq(appSettings.id, "default"))
            .returning();
          const row = yield* requireRow(rows, missingWrite("settings.saveFieldMarkers"));
          return yield* decodeSettings(row);
        },
        (effect) => mapQuery("settings.saveFieldMarkers", effect),
      ),
    });
  }),
);

const ElectronRepositoriesLive = Layer.mergeAll(
  DrizzlePlaceholderRepositoryLive,
  DrizzleTemplateRepositoryLive,
  DrizzleFirRepositoryLive,
  DrizzleFirDocumentRepositoryLive,
  DrizzleFirPlaceholderValueRepositoryLive,
  DrizzleSettingsRepositoryLive,
);

export const ElectronDatabaseLive = Layer.provideMerge(ElectronRepositoriesLive, MissalDrizzleLive);

import { Array, Cause, Effect, Layer, Match, Option, Schema } from "effect";
import { and, desc, eq } from "drizzle-orm";
import { EffectDrizzleQueryError } from "drizzle-orm/effect-core/errors";
import { isSqlError } from "effect/unstable/sql/SqlError";
import {
  appSettings,
  firPlaceholderValues,
  firRecords,
  placeholders,
  templates,
} from "#/electron/database-schema";
import { MissalDrizzle, MissalDrizzleLive } from "#/electron/database";
import { FirRecord } from "#/lib/fir";
import { Placeholder } from "#/lib/placeholder";
import { AppSettings } from "#/lib/settings";
import { EntityConflict, EntityNotFound, StorageError } from "#/lib/storage-errors";
import { FirPlaceholderValue, TemplateRecord } from "#/lib/templates";
import { nowIso } from "#/lib/time";
import {
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  SettingsRepository,
  TemplateRepository,
} from "#/repositories/index";

const sqlReason = (error: EffectDrizzleQueryError) =>
  Option.flatMap(
    Cause.isCause(error.cause) ? Cause.findErrorOption(error.cause) : Option.none(),
    (found) => (isSqlError(found) ? Option.some(found.reason) : Option.none()),
  );

const failQuery = (operation: string) => (_error: EffectDrizzleQueryError) =>
  Effect.fail(
    new StorageError({
      message: "Storage operation failed",
      operation,
    }),
  );

const decodeStored =
  <A>(decode: (value: unknown) => Effect.Effect<A, unknown>, operation: string) =>
  (value: unknown) =>
    decode(value).pipe(
      Effect.mapError(
        () =>
          new StorageError({
            message: "Stored data is invalid",
            operation,
          }),
      ),
    );

const decodeFir = (row: typeof firRecords.$inferSelect) => {
  const { templateId, ...fields } = row;
  return decodeStored(
    Schema.decodeUnknownEffect(FirRecord),
    "fir.decode",
  )({
    ...fields,
    ...(templateId == null ? {} : { templateId }),
  });
};

const firColumns = (input: {
  fir_no: string;
  date: string;
  offence: string;
  accused: string;
  witness: string;
  NIC: string;
  mobile: string;
  incident_date: string;
  arrest_date: string;
  investigation_officer: string;
  status: (typeof firRecords.$inferInsert)["status"];
  templateId?: number;
  content?: string;
}) => ({
  fir_no: input.fir_no,
  date: input.date,
  offence: input.offence,
  accused: input.accused,
  witness: input.witness,
  NIC: input.NIC,
  mobile: input.mobile,
  incident_date: input.incident_date,
  arrest_date: input.arrest_date,
  investigation_officer: input.investigation_officer,
  status: input.status,
  templateId: input.templateId ?? null,
  content: input.content ?? "",
});

export const DrizzlePlaceholderRepositoryLive = Layer.effect(
  PlaceholderRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    return PlaceholderRepository.of({
      list: Effect.gen(function* () {
        const rows = yield* db.select().from(placeholders).orderBy(placeholders.id);
        return yield* decodeStored(
          Schema.decodeUnknownEffect(Schema.Array(Placeholder)),
          "placeholder.decode",
        )(rows);
      }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("placeholder.list"))),

      create: (input) =>
        Effect.gen(function* () {
          const rows = yield* db.insert(placeholders).values(input).returning();
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new StorageError({
                  message: "Placeholder was not created",
                  operation: "placeholder.create",
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeStored(
            Schema.decodeUnknownEffect(Placeholder),
            "placeholder.decode",
          )(row);
        }).pipe(
          Effect.catchTag("EffectDrizzleQueryError", (error) =>
            Option.match(sqlReason(error), {
              onNone: () => failQuery("placeholder.create")(error),
              onSome: (reason) =>
                Match.value(reason).pipe(
                  Match.tag("UniqueViolation", () =>
                    Effect.fail(
                      new EntityConflict({
                        entity: "placeholder",
                        field: "key",
                        value: input.key,
                      }),
                    ),
                  ),
                  Match.orElse(() => failQuery("placeholder.create")(error)),
                ),
            }),
          ),
        ),

      update: (input) =>
        Effect.gen(function* () {
          const rows = yield* db
            .update(placeholders)
            .set({ key: input.key, label: input.label })
            .where(eq(placeholders.id, input.id))
            .returning();
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "placeholder",
                  id: String(input.id),
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeStored(
            Schema.decodeUnknownEffect(Placeholder),
            "placeholder.decode",
          )(row);
        }).pipe(
          Effect.catchTag("EffectDrizzleQueryError", (error) =>
            Option.match(sqlReason(error), {
              onNone: () => failQuery("placeholder.update")(error),
              onSome: (reason) =>
                Match.value(reason).pipe(
                  Match.tag("UniqueViolation", () =>
                    Effect.fail(
                      new EntityConflict({
                        entity: "placeholder",
                        field: "key",
                        value: input.key,
                      }),
                    ),
                  ),
                  Match.orElse(() => failQuery("placeholder.update")(error)),
                ),
            }),
          ),
        ),

      remove: (id) =>
        Effect.gen(function* () {
          const rows = yield* db.delete(placeholders).where(eq(placeholders.id, id)).returning();
          yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "placeholder",
                  id: String(id),
                }),
              ),
            onSome: () => Effect.void,
          });
        }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("placeholder.remove"))),
    });
  }),
);

export const DrizzleTemplateRepositoryLive = Layer.effect(
  TemplateRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    return TemplateRepository.of({
      list: Effect.gen(function* () {
        const rows = yield* db
          .select()
          .from(templates)
          .orderBy(desc(templates.updatedAt), desc(templates.id));
        return yield* decodeStored(
          Schema.decodeUnknownEffect(Schema.Array(TemplateRecord)),
          "template.decode",
        )(rows);
      }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("template.list"))),

      get: (id) =>
        Effect.gen(function* () {
          const rows = yield* db.select().from(templates).where(eq(templates.id, id));
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "template",
                  id: String(id),
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeStored(
            Schema.decodeUnknownEffect(TemplateRecord),
            "template.decode",
          )(row);
        }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("template.get"))),

      create: (input) =>
        Effect.gen(function* () {
          const timestamp = yield* nowIso;
          const rows = yield* db
            .insert(templates)
            .values({
              name: input.name,
              content: input.content,
              createdAt: timestamp,
              updatedAt: timestamp,
            })
            .returning();
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new StorageError({
                  message: "Template was not created",
                  operation: "template.create",
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeStored(
            Schema.decodeUnknownEffect(TemplateRecord),
            "template.decode",
          )(row);
        }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("template.create"))),

      update: (input) =>
        Effect.gen(function* () {
          const timestamp = yield* nowIso;
          const rows = yield* db
            .update(templates)
            .set({
              name: input.name,
              content: input.content,
              updatedAt: timestamp,
            })
            .where(eq(templates.id, input.id))
            .returning();
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "template",
                  id: String(input.id),
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeStored(
            Schema.decodeUnknownEffect(TemplateRecord),
            "template.decode",
          )(row);
        }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("template.update"))),

      remove: (id) =>
        Effect.gen(function* () {
          const rows = yield* db.delete(templates).where(eq(templates.id, id)).returning();
          yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "template",
                  id: String(id),
                }),
              ),
            onSome: () => Effect.void,
          });
        }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("template.remove"))),
    });
  }),
);

export const DrizzleFirRepositoryLive = Layer.effect(
  FirRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    return FirRepository.of({
      list: Effect.gen(function* () {
        const rows = yield* db.select().from(firRecords).orderBy(desc(firRecords.id));
        return yield* Effect.forEach(rows, decodeFir);
      }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("fir.list"))),

      get: (id) =>
        Effect.gen(function* () {
          const rows = yield* db.select().from(firRecords).where(eq(firRecords.id, id));
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "fir",
                  id: String(id),
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeFir(row);
        }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("fir.get"))),

      create: (input) =>
        Effect.gen(function* () {
          const rows = yield* db.insert(firRecords).values(firColumns(input)).returning();
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new StorageError({
                  message: "FIR was not created",
                  operation: "fir.create",
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeFir(row);
        }).pipe(
          Effect.catchTag("EffectDrizzleQueryError", (error) =>
            Option.match(sqlReason(error), {
              onNone: () => failQuery("fir.create")(error),
              onSome: (reason) =>
                Match.value(reason).pipe(
                  Match.tag("ConstraintError", () =>
                    Effect.fail(
                      new EntityNotFound({
                        entity: "template",
                        id: String(input.templateId ?? ""),
                      }),
                    ),
                  ),
                  Match.orElse(() => failQuery("fir.create")(error)),
                ),
            }),
          ),
        ),

      update: (input) =>
        Effect.gen(function* () {
          const rows = yield* db
            .update(firRecords)
            .set(firColumns(input))
            .where(eq(firRecords.id, input.id))
            .returning();
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "fir",
                  id: String(input.id),
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeFir(row);
        }).pipe(
          Effect.catchTag("EffectDrizzleQueryError", (error) =>
            Option.match(sqlReason(error), {
              onNone: () => failQuery("fir.update")(error),
              onSome: (reason) =>
                Match.value(reason).pipe(
                  Match.tag("ConstraintError", () =>
                    Effect.fail(
                      new EntityNotFound({
                        entity: "template",
                        id: String(input.templateId ?? ""),
                      }),
                    ),
                  ),
                  Match.orElse(() => failQuery("fir.update")(error)),
                ),
            }),
          ),
        ),

      updateDocument: (input) =>
        Effect.gen(function* () {
          const rows = yield* db
            .update(firRecords)
            .set({
              content: input.content,
              templateId: input.templateId ?? null,
            })
            .where(eq(firRecords.id, input.id))
            .returning();
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "fir",
                  id: String(input.id),
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeFir(row);
        }).pipe(
          Effect.catchTag("EffectDrizzleQueryError", (error) =>
            Option.match(sqlReason(error), {
              onNone: () => failQuery("fir.updateDocument")(error),
              onSome: (reason) =>
                Match.value(reason).pipe(
                  Match.tag("ConstraintError", () =>
                    Effect.fail(
                      new EntityNotFound({
                        entity: "template",
                        id: String(input.templateId ?? ""),
                      }),
                    ),
                  ),
                  Match.orElse(() => failQuery("fir.updateDocument")(error)),
                ),
            }),
          ),
        ),

      remove: (id) =>
        Effect.gen(function* () {
          const rows = yield* db.delete(firRecords).where(eq(firRecords.id, id)).returning();
          yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "fir",
                  id: String(id),
                }),
              ),
            onSome: () => Effect.void,
          });
        }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("fir.remove"))),
    });
  }),
);

export const DrizzleFirPlaceholderValueRepositoryLive = Layer.effect(
  FirPlaceholderValueRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    return FirPlaceholderValueRepository.of({
      listForFir: (firId) =>
        Effect.gen(function* () {
          const rows = yield* db
            .select()
            .from(firPlaceholderValues)
            .where(eq(firPlaceholderValues.firId, firId));
          return yield* decodeStored(
            Schema.decodeUnknownEffect(Schema.Array(FirPlaceholderValue)),
            "firPlaceholderValue.decode",
          )(rows);
        }).pipe(
          Effect.catchTag("EffectDrizzleQueryError", failQuery("firPlaceholderValue.listForFir")),
        ),

      upsert: (input) =>
        db
          .transaction(
            Effect.fnUntraced(function* (tx) {
              const fir = yield* tx.select().from(firRecords).where(eq(firRecords.id, input.firId));
              yield* Option.match(Array.head(fir), {
                onNone: () =>
                  Effect.fail(
                    new EntityNotFound({
                      entity: "fir",
                      id: String(input.firId),
                    }),
                  ),
                onSome: () => Effect.void,
              });

              const placeholder = yield* tx
                .select()
                .from(placeholders)
                .where(eq(placeholders.id, input.placeholderId));
              yield* Option.match(Array.head(placeholder), {
                onNone: () =>
                  Effect.fail(
                    new EntityNotFound({
                      entity: "placeholder",
                      id: String(input.placeholderId),
                    }),
                  ),
                onSome: () => Effect.void,
              });

              const timestamp = yield* nowIso;
              const rows = yield* tx
                .insert(firPlaceholderValues)
                .values({
                  firId: input.firId,
                  placeholderId: input.placeholderId,
                  value: input.value,
                  updatedAt: timestamp,
                })
                .onConflictDoUpdate({
                  target: [firPlaceholderValues.firId, firPlaceholderValues.placeholderId],
                  set: {
                    value: input.value,
                    updatedAt: timestamp,
                  },
                })
                .returning();
              const row = yield* Option.match(Array.head(rows), {
                onNone: () =>
                  Effect.fail(
                    new StorageError({
                      message: "Placeholder value was not saved",
                      operation: "firPlaceholderValue.upsert",
                    }),
                  ),
                onSome: (value) => Effect.succeed(value),
              });
              return yield* decodeStored(
                Schema.decodeUnknownEffect(FirPlaceholderValue),
                "firPlaceholderValue.decode",
              )(row);
            }),
          )
          .pipe(
            Effect.catchTag("EffectDrizzleQueryError", failQuery("firPlaceholderValue.upsert")),
            Effect.catchTag("SqlError", () =>
              Effect.fail(
                new StorageError({
                  message: "Storage operation failed",
                  operation: "firPlaceholderValue.upsert",
                }),
              ),
            ),
          ),

      remove: (input) =>
        Effect.gen(function* () {
          const rows = yield* db
            .delete(firPlaceholderValues)
            .where(
              and(
                eq(firPlaceholderValues.firId, input.firId),
                eq(firPlaceholderValues.placeholderId, input.placeholderId),
              ),
            )
            .returning();
          yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new EntityNotFound({
                  entity: "firPlaceholderValue",
                  id: `${input.firId}:${input.placeholderId}`,
                }),
              ),
            onSome: () => Effect.void,
          });
        }).pipe(
          Effect.catchTag("EffectDrizzleQueryError", failQuery("firPlaceholderValue.remove")),
        ),
    });
  }),
);

export const DrizzleSettingsRepositoryLive = Layer.effect(
  SettingsRepository,
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    return SettingsRepository.of({
      get: Effect.gen(function* () {
        const rows = yield* db.select().from(appSettings).where(eq(appSettings.id, "default"));
        const row = yield* Option.match(Array.head(rows), {
          onNone: () =>
            Effect.fail(
              new StorageError({
                message: "Settings are missing",
                operation: "settings.get",
              }),
            ),
          onSome: (value) => Effect.succeed(value),
        });
        return yield* decodeStored(Schema.decodeUnknownEffect(AppSettings), "settings.decode")(row);
      }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("settings.get"))),

      save: (sharedPlaceholders) =>
        Effect.gen(function* () {
          const timestamp = yield* nowIso;
          const rows = yield* db
            .update(appSettings)
            .set({
              sharedPlaceholders: { ...sharedPlaceholders },
              updatedAt: timestamp,
            })
            .where(eq(appSettings.id, "default"))
            .returning();
          const row = yield* Option.match(Array.head(rows), {
            onNone: () =>
              Effect.fail(
                new StorageError({
                  message: "Settings are missing",
                  operation: "settings.save",
                }),
              ),
            onSome: (value) => Effect.succeed(value),
          });
          return yield* decodeStored(
            Schema.decodeUnknownEffect(AppSettings),
            "settings.decode",
          )(row);
        }).pipe(Effect.catchTag("EffectDrizzleQueryError", failQuery("settings.save"))),
    });
  }),
);

export const ElectronRepositoriesLive = Layer.mergeAll(
  DrizzlePlaceholderRepositoryLive,
  DrizzleTemplateRepositoryLive,
  DrizzleFirRepositoryLive,
  DrizzleFirPlaceholderValueRepositoryLive,
  DrizzleSettingsRepositoryLive,
);

export const ElectronDatabaseLive = Layer.provideMerge(ElectronRepositoriesLive, MissalDrizzleLive);

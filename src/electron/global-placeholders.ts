import { randomUUID } from "node:crypto";
import { Effect, Schema } from "effect";
import { eq } from "drizzle-orm";
import type { MissalDrizzle } from "#/electron/database";
import { placeholders } from "#/electron/database-schema";
import {
  decodeStored,
  mapQuery,
  mapTransaction,
  notFound,
  readSettings,
  recoverUnique,
  requireRow,
  writeSettings,
} from "#/electron/repository-helpers";
import { GlobalPlaceholder, SaveGlobalPlaceholdersInput } from "#/lib/global-placeholder";
import { SharedSettingSource } from "#/lib/field";
import { Placeholder } from "#/lib/placeholder";
import { StorageError } from "#/lib/storage-errors";

export function globalPlaceholderOperations(db: MissalDrizzle["Service"]) {
  const listGlobals = Effect.gen(function* () {
    const rows = yield* db.select().from(placeholders).orderBy(placeholders.id);
    const catalog = yield* decodeStored(
      Schema.decodeUnknownEffect(Schema.Array(Placeholder)),
      "globalPlaceholder.list",
    )(rows);
    const settings = yield* readSettings(db);
    return catalog.flatMap((field) =>
      field.source._tag === "SharedSetting"
        ? [
            new GlobalPlaceholder({
              id: field.id,
              label: field.label,
              value: settings.sharedPlaceholders[field.source.setting] ?? "",
            }),
          ]
        : [],
    );
  }).pipe((effect) => mapQuery("globalPlaceholder.list", effect));

  const saveGlobals = Effect.fn("PlaceholderRepository.saveGlobals")(
    function* (input: SaveGlobalPlaceholdersInput) {
      yield* db.transaction(
        Effect.fnUntraced(function* (tx) {
          const settings = yield* readSettings(tx);
          const values = { ...settings.sharedPlaceholders };
          for (const entry of input.entries) {
            if (entry._tag === "New") {
              const setting = `global_${randomUUID().replaceAll("-", "_")}`;
              yield* tx
                .insert(placeholders)
                .values({ label: entry.label, source: SharedSettingSource.make({ setting }) })
                .pipe(
                  Effect.catchTag(
                    "EffectDrizzleQueryError",
                    recoverUnique("globalPlaceholder.save", "placeholder", "name", entry.label),
                  ),
                );
              values[setting] = entry.value;
            } else {
              const rows = yield* tx
                .select()
                .from(placeholders)
                .where(eq(placeholders.id, entry.id));
              const field = yield* decodeStored(
                Schema.decodeUnknownEffect(Placeholder),
                "globalPlaceholder.decode",
              )(yield* requireRow(rows, notFound("placeholder", entry.id)));
              if (field.source._tag !== "SharedSetting") {
                return yield* new StorageError({
                  operation: "globalPlaceholder.save",
                  message: "This field belongs to an FIR, not global values.",
                });
              }
              yield* tx
                .update(placeholders)
                .set({ label: entry.label })
                .where(eq(placeholders.id, entry.id))
                .pipe(
                  Effect.catchTag(
                    "EffectDrizzleQueryError",
                    recoverUnique("globalPlaceholder.save", "placeholder", "name", entry.label),
                  ),
                );
              values[field.source.setting] = entry.value;
            }
          }
          yield* writeSettings(tx, { sharedPlaceholders: values });
        }),
      );
      return yield* listGlobals;
    },
    (effect) => mapTransaction("globalPlaceholder.save", effect),
  );
  return { listGlobals, saveGlobals };
}

import { randomUUID } from "node:crypto";
import { Effect, Schema } from "effect";
import { eq } from "drizzle-orm";
import type { MissalDrizzle } from "#/electron/database";
import { appSettings, placeholders } from "#/electron/database-schema";
import {
  decodeStored,
  mapQuery,
  mapTransaction,
  missingWrite,
  notFound,
  requireRow,
} from "#/electron/repository-helpers";
import { GlobalPlaceholder, SaveGlobalPlaceholdersInput } from "#/lib/global-placeholder";
import { SharedSettingSource } from "#/lib/field";
import { Placeholder, PlaceholderKey } from "#/lib/placeholder";
import { AppSettings } from "#/lib/settings";
import { StorageError } from "#/lib/storage-errors";
import { nowIso } from "#/lib/time";

export function globalPlaceholderOperations(db: MissalDrizzle["Service"]) {
  const listGlobals = Effect.gen(function* () {
    const rows = yield* db.select().from(placeholders).orderBy(placeholders.id);
    const catalog = yield* decodeStored(
      Schema.decodeUnknownEffect(Schema.Array(Placeholder)),
      "globalPlaceholder.list",
    )(rows);
    const settingsRows = yield* db.select().from(appSettings).where(eq(appSettings.id, "default"));
    const settings = yield* decodeStored(
      Schema.decodeUnknownEffect(AppSettings),
      "globalPlaceholder.settings",
    )(yield* requireRow(settingsRows, missingWrite("settings.get")));
    return catalog.flatMap((field) =>
      field.source._tag === "SharedSetting"
        ? [
            new GlobalPlaceholder({
              id: field.id,
              key: field.key,
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
          const settingsRows = yield* tx
            .select()
            .from(appSettings)
            .where(eq(appSettings.id, "default"));
          const settings = yield* decodeStored(
            Schema.decodeUnknownEffect(AppSettings),
            "globalPlaceholder.settings",
          )(yield* requireRow(settingsRows, missingWrite("settings.get")));
          const values = { ...settings.sharedPlaceholders };
          for (const entry of input.entries) {
            if (entry._tag === "New") {
              const key = PlaceholderKey.make(`global_${randomUUID().replaceAll("-", "_")}`);
              yield* tx.insert(placeholders).values({
                key,
                label: entry.label,
                source: SharedSettingSource.make({ setting: key }),
              });
              values[key] = entry.value;
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
                .where(eq(placeholders.id, entry.id));
              values[field.source.setting] = entry.value;
            }
          }
          yield* tx
            .update(appSettings)
            .set({ sharedPlaceholders: values, updatedAt: yield* nowIso })
            .where(eq(appSettings.id, "default"));
        }),
      );
      return yield* listGlobals;
    },
    (effect) => mapTransaction("globalPlaceholder.save", effect),
  );
  return { listGlobals, saveGlobals };
}

import { randomUUID } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { gunzipSync } from "node:zlib";
import { Context, Effect, Layer, Schema } from "effect";
import { appSettings, placeholders, templates } from "#/electron/database-schema";
import { MissalDrizzle } from "#/electron/database";
import { decodeStored, mapTransaction } from "#/electron/repository-helpers";
import { DocumentEnvelope, documentWriteColumns } from "#/lib/document-format";
import { CustomSource, SharedSettingSource } from "#/lib/field";
import { PlaceholderId } from "#/lib/ids";
import { fieldLookupKey } from "#/lib/placeholder";
import { StorageError } from "#/lib/storage-errors";
import { BundledTemplates, TemplateBundleFields, bindCatalogFields } from "#/lib/template-bundle";
import { nowIso } from "#/lib/time";

export class TemplatesFolder extends Context.Service<TemplatesFolder, string>()(
  "missal/TemplatesFolder",
) {}

const readJson = (file: string, gzipped: boolean) =>
  Effect.tryPromise({
    try: async () => {
      const bytes = await readFile(file);
      return JSON.parse((gzipped ? gunzipSync(bytes) : bytes).toString("utf8")) as unknown;
    },
    catch: () =>
      new StorageError({
        message: "Bundled templates are missing",
        operation: "bundledTemplates.read",
      }),
  });

export const BundledTemplatesLive = Layer.effectDiscard(
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;
    const folder = yield* TemplatesFolder;
    const fields = yield* readJson(path.join(folder, "fields.json"), false).pipe(
      Effect.flatMap(
        decodeStored(Schema.decodeUnknownEffect(TemplateBundleFields), "bundledTemplates.decode"),
      ),
    );

    yield* db.transaction(
      Effect.fnUntraced(function* (tx) {
        const [settings] = yield* tx
          .select({ version: appSettings.bundledTemplatesVersion })
          .from(appSettings);
        if ((settings?.version ?? 0) >= fields.version) return;

        const bundle = yield* readJson(path.join(folder, "bundle.json.gz"), true).pipe(
          Effect.flatMap(
            decodeStored(Schema.decodeUnknownEffect(BundledTemplates), "bundledTemplates.decode"),
          ),
        );

        const ids = new Map<string, PlaceholderId>();
        for (const row of yield* tx
          .select({ id: placeholders.id, label: placeholders.label })
          .from(placeholders))
          ids.set(fieldLookupKey(row.label), PlaceholderId.make(row.id));

        const sources = [
          ...fields.global.map(
            (label) =>
              [
                label,
                SharedSettingSource.make({
                  setting: `global_${randomUUID().replaceAll("-", "_")}`,
                }),
              ] as const,
          ),
          ...fields.custom.map((label) => [label, CustomSource.make({})] as const),
        ];
        for (const [label, source] of sources) {
          if (ids.has(fieldLookupKey(label))) continue;
          const [row] = yield* tx
            .insert(placeholders)
            .values({ label, source })
            .returning({ id: placeholders.id });
          if (row) ids.set(fieldLookupKey(label), PlaceholderId.make(row.id));
        }

        const existing = new Set(
          (yield* tx.select({ name: templates.name }).from(templates)).map(({ name }) => name),
        );
        const timestamp = yield* nowIso;
        for (const template of bundle) {
          if (existing.has(template.name)) continue;
          yield* tx.insert(templates).values({
            createdAt: timestamp,
            name: template.name,
            revision: 1,
            updatedAt: timestamp,
            ...documentWriteColumns(
              new DocumentEnvelope({
                format: template.document.format,
                version: template.document.version,
                state: bindCatalogFields(template.document.state, ids),
                ...(template.document.pageLayout
                  ? { pageLayout: template.document.pageLayout }
                  : {}),
              }),
            ),
          });
        }

        yield* tx
          .insert(appSettings)
          .values({ bundledTemplatesVersion: fields.version })
          .onConflictDoUpdate({
            target: appSettings.id,
            set: { bundledTemplatesVersion: fields.version },
          });
      }),
    );
  }).pipe((effect) => mapTransaction("bundledTemplates.install", effect)),
);

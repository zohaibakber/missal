import { Effect, Layer, Schema } from "effect";
import { eq, sql } from "drizzle-orm";
import { firDocuments, templates } from "#/electron/database-schema";
import { MissalDrizzle } from "#/electron/database";
import { decodeStored, mapTransaction } from "#/electron/repository-helpers";
import { DOCUMENT_FORMAT_VERSION, StoredDocumentEnvelope } from "#/lib/document-format";

const decodeStoredEnvelope = decodeStored(
  Schema.decodeUnknownEffect(StoredDocumentEnvelope),
  "documentFormat.upgrade.decode",
);

export const DocumentFormatUpgradeLive = Layer.effectDiscard(
  Effect.gen(function* () {
    const db = yield* MissalDrizzle;

    yield* db.transaction(
      Effect.fnUntraced(function* (tx) {
        const staleTemplates = yield* tx
          .select({ id: templates.id, document: templates.document })
          .from(templates)
          .where(
            sql`json_extract(${templates.document}, '$.version') < ${DOCUMENT_FORMAT_VERSION}`,
          );
        yield* Effect.forEach(
          staleTemplates,
          ({ id, document }) =>
            decodeStoredEnvelope(document).pipe(
              Effect.flatMap((envelope) =>
                tx.update(templates).set({ document: envelope }).where(eq(templates.id, id)),
              ),
            ),
          { discard: true },
        );

        const staleDocuments = yield* tx
          .select({ id: firDocuments.id, document: firDocuments.document })
          .from(firDocuments)
          .where(
            sql`json_extract(${firDocuments.document}, '$.version') < ${DOCUMENT_FORMAT_VERSION}`,
          );
        yield* Effect.forEach(
          staleDocuments,
          ({ id, document }) =>
            decodeStoredEnvelope(document).pipe(
              Effect.flatMap((envelope) =>
                tx.update(firDocuments).set({ document: envelope }).where(eq(firDocuments.id, id)),
              ),
            ),
          { discard: true },
        );
      }),
    );
  }).pipe((effect) => mapTransaction("documentFormat.upgrade", effect)),
);

import { createCollection, localStorageCollectionOptions } from "@tanstack/react-db";
import type { FirRecord } from "#/lib/fir";
import { firSchema } from "#/lib/fir";

export const FIR_COLLECTION_ID = "fir-records";
export const FIR_STORAGE_KEY = "missal-vite.fir-records";

export const firCollection = createCollection(
  localStorageCollectionOptions({
    id: FIR_COLLECTION_ID,
    storageKey: FIR_STORAGE_KEY,
    getKey: (fir: FirRecord) => fir.id,
    schema: firSchema,
  }),
);

export function getNextFirId(records: FirRecord[]) {
  if (!records.length) {
    return 1;
  }

  return Math.max(...records.map((record) => record.id)) + 1;
}

export function replaceAllFirRecords(records: FirRecord[]) {
  for (const record of Array.from(firCollection.state.values())) {
    firCollection.delete(record.id);
  }

  for (const record of records) {
    firCollection.insert(record);
  }
}

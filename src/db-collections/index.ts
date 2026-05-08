import { createCollection, localStorageCollectionOptions } from "@tanstack/react-db";
import type { FirRecord } from "#/lib/fir";
import { firSchema } from "#/lib/fir";
import type { AppSettings } from "#/lib/settings";
import {
  appSettingsSchema,
  createDefaultAppSettings,
  normalizeSharedPlaceholderSettings,
} from "#/lib/settings";
import type { FirPlaceholderValue, TemplateRecord } from "#/lib/templates";
import {
  firPlaceholderValueSchema,
  getPlaceholderValueId,
  templateRecordSchema,
} from "#/lib/templates";

export const FIR_COLLECTION_ID = "fir-records";
export const FIR_STORAGE_KEY = "missal-vite.fir-records";
export const TEMPLATE_COLLECTION_ID = "templates";
export const TEMPLATE_STORAGE_KEY = "missal-vite.templates";
export const FIR_PLACEHOLDER_COLLECTION_ID = "fir-placeholder-values";
export const FIR_PLACEHOLDER_STORAGE_KEY = "missal-vite.fir-placeholder-values";
export const APP_SETTINGS_COLLECTION_ID = "app-settings";
export const APP_SETTINGS_STORAGE_KEY = "missal-vite.app-settings";

export const firCollection = createCollection(
  localStorageCollectionOptions({
    id: FIR_COLLECTION_ID,
    storageKey: FIR_STORAGE_KEY,
    getKey: (fir: FirRecord) => fir.id,
    schema: firSchema,
  }),
);

export const templateCollection = createCollection(
  localStorageCollectionOptions({
    id: TEMPLATE_COLLECTION_ID,
    storageKey: TEMPLATE_STORAGE_KEY,
    getKey: (template: TemplateRecord) => template.id,
    schema: templateRecordSchema,
  }),
);

export const firPlaceholderValueCollection = createCollection(
  localStorageCollectionOptions({
    id: FIR_PLACEHOLDER_COLLECTION_ID,
    storageKey: FIR_PLACEHOLDER_STORAGE_KEY,
    getKey: (value: FirPlaceholderValue) => value.id,
    schema: firPlaceholderValueSchema,
  }),
);

export const appSettingsCollection = createCollection(
  localStorageCollectionOptions({
    id: APP_SETTINGS_COLLECTION_ID,
    storageKey: APP_SETTINGS_STORAGE_KEY,
    getKey: (settings: AppSettings) => settings.id,
    schema: appSettingsSchema,
  }),
);

export function getAppSettings(records: AppSettings[]) {
  const settings = records.find((record) => record.id === "default") ?? createDefaultAppSettings();

  return {
    ...settings,
    sharedPlaceholders: normalizeSharedPlaceholderSettings(settings.sharedPlaceholders),
  };
}

export function saveAppSettings(settings: AppSettings) {
  if (appSettingsCollection.state.has(settings.id)) {
    appSettingsCollection.update(settings.id, (draft) => {
      draft.sharedPlaceholders = settings.sharedPlaceholders;
      draft.updatedAt = settings.updatedAt;
    });
    return;
  }

  appSettingsCollection.insert(settings);
}

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

export function getNextTemplateId(records: TemplateRecord[]) {
  if (!records.length) {
    return 1;
  }

  return Math.max(...records.map((record) => record.id)) + 1;
}

export function upsertFirPlaceholderValue({
  firId,
  placeholder,
  value,
}: {
  firId: number;
  placeholder: string;
  value: string;
}) {
  const id = getPlaceholderValueId(firId, placeholder);
  const record: FirPlaceholderValue = {
    id,
    firId,
    placeholder,
    value,
    updatedAt: new Date().toISOString(),
  };

  if (firPlaceholderValueCollection.state.has(id)) {
    firPlaceholderValueCollection.update(id, (draft) => {
      draft.value = value;
      draft.updatedAt = record.updatedAt;
    });
    return;
  }

  firPlaceholderValueCollection.insert(record);
}

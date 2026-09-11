import { Schema } from "effect";
import {
  FirCreateInput,
  FirDocumentUpdateInput,
  FirId,
  FirRecord,
  FirUpdateInput,
} from "#/lib/fir";
import { PlaceholderId } from "#/lib/ids";
import { Placeholder, PlaceholderCreateInput, PlaceholderUpdateInput } from "#/lib/placeholder";
import { RepositoryError } from "#/lib/storage-errors";
import {
  FirPlaceholderValue,
  FirPlaceholderValueRemoveInput,
  FirPlaceholderValueUpsertInput,
  TemplateCreateInput,
  TemplateId,
  TemplateRecord,
  TemplateUpdateInput,
} from "#/lib/templates";

export const STORAGE_CHANNEL = "app-storage:request";

export const PlaceholderListRequest = Schema.TaggedStruct("Placeholder.list", {});
export const PlaceholderCreateRequest = Schema.TaggedStruct("Placeholder.create", {
  input: PlaceholderCreateInput,
});
export const PlaceholderUpdateRequest = Schema.TaggedStruct("Placeholder.update", {
  input: PlaceholderUpdateInput,
});
export const PlaceholderRemoveRequest = Schema.TaggedStruct("Placeholder.remove", {
  id: PlaceholderId,
});

export const TemplateListRequest = Schema.TaggedStruct("Template.list", {});
export const TemplateGetRequest = Schema.TaggedStruct("Template.get", {
  id: TemplateId,
});
export const TemplateCreateRequest = Schema.TaggedStruct("Template.create", {
  input: TemplateCreateInput,
});
export const TemplateUpdateRequest = Schema.TaggedStruct("Template.update", {
  input: TemplateUpdateInput,
});
export const TemplateRemoveRequest = Schema.TaggedStruct("Template.remove", {
  id: TemplateId,
});

export const FirListRequest = Schema.TaggedStruct("Fir.list", {});
export const FirGetRequest = Schema.TaggedStruct("Fir.get", {
  id: FirId,
});
export const FirCreateRequest = Schema.TaggedStruct("Fir.create", {
  input: FirCreateInput,
});
export const FirUpdateRequest = Schema.TaggedStruct("Fir.update", {
  input: FirUpdateInput,
});
export const FirUpdateDocumentRequest = Schema.TaggedStruct("Fir.updateDocument", {
  input: FirDocumentUpdateInput,
});
export const FirRemoveRequest = Schema.TaggedStruct("Fir.remove", {
  id: FirId,
});

export const FirPlaceholderValueListRequest = Schema.TaggedStruct(
  "FirPlaceholderValue.listForFir",
  {
    firId: FirId,
  },
);
export const FirPlaceholderValueUpsertRequest = Schema.TaggedStruct("FirPlaceholderValue.upsert", {
  input: FirPlaceholderValueUpsertInput,
});
export const FirPlaceholderValueRemoveRequest = Schema.TaggedStruct("FirPlaceholderValue.remove", {
  input: FirPlaceholderValueRemoveInput,
});

export const SettingsGetRequest = Schema.TaggedStruct("Settings.get", {});
export const SettingsSaveRequest = Schema.TaggedStruct("Settings.save", {
  sharedPlaceholders: Schema.Record(Schema.String, Schema.String),
});

export const StorageRequest = Schema.Union([
  PlaceholderListRequest,
  PlaceholderCreateRequest,
  PlaceholderUpdateRequest,
  PlaceholderRemoveRequest,
  TemplateListRequest,
  TemplateGetRequest,
  TemplateCreateRequest,
  TemplateUpdateRequest,
  TemplateRemoveRequest,
  FirListRequest,
  FirGetRequest,
  FirCreateRequest,
  FirUpdateRequest,
  FirUpdateDocumentRequest,
  FirRemoveRequest,
  FirPlaceholderValueListRequest,
  FirPlaceholderValueUpsertRequest,
  FirPlaceholderValueRemoveRequest,
  SettingsGetRequest,
  SettingsSaveRequest,
]);

export type StorageRequest = typeof StorageRequest.Type;

export const StorageResponse = Schema.Exit(Schema.Unknown, RepositoryError, Schema.Defect());

export type StorageResponse = typeof StorageResponse.Type;

export const PlaceholderListResult = Schema.Array(Placeholder);
export const TemplateListResult = Schema.Array(TemplateRecord);
export const FirListResult = Schema.Array(FirRecord);
export const FirPlaceholderValueListResult = Schema.Array(FirPlaceholderValue);

export const VoidResult = Schema.Undefined;

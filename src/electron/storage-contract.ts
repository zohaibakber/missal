import { GlobalPlaceholder, SaveGlobalPlaceholdersInput } from "#/lib/global-placeholder";
import { Schema } from "effect";
import {
  AddFirTemplatesInput,
  FirDocumentRecord,
  FirDocumentSaveAck,
  FirDocumentSaveInput,
  FirDocumentSummary,
  FirValueContext,
  ReorderFirDocumentsInput,
} from "#/lib/fir-document";
import { FirCreateInput, FirId, FirRecord, FirUpdateInput } from "#/lib/fir";
import { FirDocumentId, PlaceholderId, TemplateId } from "#/lib/ids";
import { Placeholder, PlaceholderCreateInput, PlaceholderUpdateInput } from "#/lib/placeholder";
import { RepositoryError } from "#/lib/storage-errors";
import { FieldMarkers } from "#/lib/settings";
import {
  FirPlaceholderValue,
  FirPlaceholderValueRemoveInput,
  FirPlaceholderValueUpsertInput,
  TemplateCreateInput,
  TemplateRecord,
  TemplateSaveAck,
  TemplateSummary,
  TemplateUpdateInput,
} from "#/lib/templates";

export const GlobalPlaceholderListRequest = Schema.TaggedStruct("Placeholder.listGlobals", {});
export const GlobalPlaceholderSaveRequest = Schema.TaggedStruct("Placeholder.saveGlobals", {
  input: SaveGlobalPlaceholdersInput,
});
export const GlobalPlaceholderListResult = Schema.Array(GlobalPlaceholder);

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
export const TemplateSearchRequest = Schema.TaggedStruct("Template.search", {
  query: Schema.String,
});
export const TemplateGetRequest = Schema.TaggedStruct("Template.get", {
  id: TemplateId,
});
export const TemplateCreateRequest = Schema.TaggedStruct("Template.create", {
  input: TemplateCreateInput,
});
export const TemplateSaveRequest = Schema.TaggedStruct("Template.save", {
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
export const FirRemoveRequest = Schema.TaggedStruct("Fir.remove", {
  id: FirId,
});
export const FirValueContextRequest = Schema.TaggedStruct("Fir.valueContext", {
  id: FirId,
});

export const FirDocumentListRequest = Schema.TaggedStruct("FirDocument.listForFir", {
  firId: FirId,
});
export const FirDocumentGetRequest = Schema.TaggedStruct("FirDocument.get", {
  id: FirDocumentId,
});
export const FirDocumentAddTemplatesRequest = Schema.TaggedStruct("FirDocument.addTemplates", {
  input: AddFirTemplatesInput,
});
export const FirDocumentSaveRequest = Schema.TaggedStruct("FirDocument.save", {
  input: FirDocumentSaveInput,
});
export const FirDocumentReorderRequest = Schema.TaggedStruct("FirDocument.reorder", {
  input: ReorderFirDocumentsInput,
});
export const FirDocumentRemoveRequest = Schema.TaggedStruct("FirDocument.remove", {
  id: FirDocumentId,
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
export const SettingsSaveFieldMarkersRequest = Schema.TaggedStruct("Settings.saveFieldMarkers", {
  fieldMarkers: FieldMarkers,
});

export const StorageRequest = Schema.Union([
  GlobalPlaceholderListRequest,
  GlobalPlaceholderSaveRequest,
  PlaceholderListRequest,
  PlaceholderCreateRequest,
  PlaceholderUpdateRequest,
  PlaceholderRemoveRequest,
  TemplateListRequest,
  TemplateSearchRequest,
  TemplateGetRequest,
  TemplateCreateRequest,
  TemplateSaveRequest,
  TemplateRemoveRequest,
  FirListRequest,
  FirGetRequest,
  FirCreateRequest,
  FirUpdateRequest,
  FirRemoveRequest,
  FirValueContextRequest,
  FirDocumentListRequest,
  FirDocumentGetRequest,
  FirDocumentAddTemplatesRequest,
  FirDocumentSaveRequest,
  FirDocumentReorderRequest,
  FirDocumentRemoveRequest,
  FirPlaceholderValueListRequest,
  FirPlaceholderValueUpsertRequest,
  FirPlaceholderValueRemoveRequest,
  SettingsGetRequest,
  SettingsSaveRequest,
  SettingsSaveFieldMarkersRequest,
]);

export type StorageRequest = typeof StorageRequest.Type;

const StorageSuccessResponse = Schema.TaggedStruct("Success", {
  value: Schema.Unknown,
});
const StorageFailureResponse = Schema.TaggedStruct("Failure", {
  error: RepositoryError,
});
export const StorageResponse = Schema.Union([StorageSuccessResponse, StorageFailureResponse]);

export type StorageResponse = typeof StorageResponse.Type;

export const encodeStorageResponse = Schema.encodeUnknownSync(StorageResponse);
export const decodeStorageResponse = Schema.decodeUnknownEffect(StorageResponse);

export const PlaceholderListResult = Schema.Array(Placeholder);
export const TemplateListResult = Schema.Array(TemplateSummary);
export const TemplateRecordResult = TemplateRecord;
export const TemplateSaveAckResult = TemplateSaveAck;
export const FirListResult = Schema.Array(FirRecord);
export const FirDocumentListResult = Schema.Array(FirDocumentSummary);
export const FirDocumentRecordResult = FirDocumentRecord;
export const FirDocumentSaveAckResult = FirDocumentSaveAck;
export const FirPlaceholderValueListResult = Schema.Array(FirPlaceholderValue);
export const FirValueContextResult = FirValueContext;

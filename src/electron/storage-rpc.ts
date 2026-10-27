import { Schema } from "effect";
import { Rpc, RpcGroup } from "effect/rpc";
import {
  AddFirTemplatesInput,
  FirDocumentRecord,
  FirDocumentSaveAck,
  FirDocumentSaveInput,
  FirDocumentSummary,
  FirValueContext,
  ReorderFirDocumentsInput,
} from "#/lib/fir-document";
import { FirCreateInput, FirRecord, FirSummary, FirUpdateInput } from "#/lib/fir";
import { GlobalPlaceholder, SaveGlobalPlaceholdersInput } from "#/lib/global-placeholder";
import { FirDocumentId, FirId, PlaceholderId, TemplateId } from "#/lib/ids";
import { Placeholder, PlaceholderCreateInput, PlaceholderUpdateInput } from "#/lib/placeholder";
import { AppSettings, FieldMarkers } from "#/lib/settings";
import { RepositoryError } from "#/lib/storage-errors";
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

export class StorageRpcs extends RpcGroup.make(
  Rpc.make("Placeholder.listGlobals", {
    success: Schema.Array(GlobalPlaceholder),
    error: RepositoryError,
  }),
  Rpc.make("Placeholder.saveGlobals", {
    payload: SaveGlobalPlaceholdersInput,
    success: Schema.Array(GlobalPlaceholder),
    error: RepositoryError,
  }),
  Rpc.make("Placeholder.list", { success: Schema.Array(Placeholder), error: RepositoryError }),
  Rpc.make("Placeholder.create", {
    payload: PlaceholderCreateInput,
    success: Placeholder,
    error: RepositoryError,
  }),
  Rpc.make("Placeholder.update", {
    payload: PlaceholderUpdateInput,
    success: Placeholder,
    error: RepositoryError,
  }),
  Rpc.make("Placeholder.remove", { payload: PlaceholderId, error: RepositoryError }),

  Rpc.make("Template.list", { success: Schema.Array(TemplateSummary), error: RepositoryError }),
  Rpc.make("Template.search", {
    payload: Schema.String,
    success: Schema.Array(TemplateSummary),
    error: RepositoryError,
  }),
  Rpc.make("Template.get", {
    payload: TemplateId,
    success: TemplateRecord,
    error: RepositoryError,
  }),
  Rpc.make("Template.create", {
    payload: TemplateCreateInput,
    success: TemplateSummary,
    error: RepositoryError,
  }),
  Rpc.make("Template.save", {
    payload: TemplateUpdateInput,
    success: TemplateSaveAck,
    error: RepositoryError,
  }),
  Rpc.make("Template.remove", { payload: TemplateId, error: RepositoryError }),

  Rpc.make("Fir.list", { success: Schema.Array(FirSummary), error: RepositoryError }),
  Rpc.make("Fir.get", { payload: FirId, success: FirRecord, error: RepositoryError }),
  Rpc.make("Fir.create", { payload: FirCreateInput, success: FirRecord, error: RepositoryError }),
  Rpc.make("Fir.update", { payload: FirUpdateInput, success: FirRecord, error: RepositoryError }),
  Rpc.make("Fir.remove", { payload: FirId, error: RepositoryError }),
  Rpc.make("Fir.valueContext", {
    payload: FirId,
    success: FirValueContext,
    error: RepositoryError,
  }),

  Rpc.make("FirDocument.listForFir", {
    payload: FirId,
    success: Schema.Array(FirDocumentSummary),
    error: RepositoryError,
  }),
  Rpc.make("FirDocument.get", {
    payload: FirDocumentId,
    success: FirDocumentRecord,
    error: RepositoryError,
  }),
  Rpc.make("FirDocument.getMany", {
    payload: Schema.Array(FirDocumentId),
    success: Schema.Array(FirDocumentRecord),
    error: RepositoryError,
  }),
  Rpc.make("FirDocument.addTemplates", {
    payload: AddFirTemplatesInput,
    success: Schema.Array(FirDocumentSummary),
    error: RepositoryError,
  }),
  Rpc.make("FirDocument.save", {
    payload: FirDocumentSaveInput,
    success: FirDocumentSaveAck,
    error: RepositoryError,
  }),
  Rpc.make("FirDocument.reorder", {
    payload: ReorderFirDocumentsInput,
    success: Schema.Array(FirDocumentSummary),
    error: RepositoryError,
  }),
  Rpc.make("FirDocument.remove", { payload: FirDocumentId, error: RepositoryError }),

  Rpc.make("FirPlaceholderValue.listForFir", {
    payload: FirId,
    success: Schema.Array(FirPlaceholderValue),
    error: RepositoryError,
  }),
  Rpc.make("FirPlaceholderValue.upsert", {
    payload: FirPlaceholderValueUpsertInput,
    success: FirPlaceholderValue,
    error: RepositoryError,
  }),
  Rpc.make("FirPlaceholderValue.remove", {
    payload: FirPlaceholderValueRemoveInput,
    error: RepositoryError,
  }),

  Rpc.make("Settings.get", { success: AppSettings, error: RepositoryError }),
  Rpc.make("Settings.save", {
    payload: Schema.Record(Schema.String, Schema.String),
    success: AppSettings,
    error: RepositoryError,
  }),
  Rpc.make("Settings.saveFieldMarkers", {
    payload: FieldMarkers,
    success: AppSettings,
    error: RepositoryError,
  }),
) {}

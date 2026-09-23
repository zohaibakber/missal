import { Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";
import { FirId, PlaceholderId, TemplateId } from "#/lib/ids";
import { DocumentRevision } from "#/lib/ids";
import { IsoDateTimeString, NonEmptyTrimmedString } from "#/lib/schema";

export { TemplateId } from "#/lib/ids";

export class TemplateSummary extends Schema.Class<TemplateSummary>("TemplateSummary")({
  id: TemplateId,
  name: NonEmptyTrimmedString,
  revision: DocumentRevision,
  previewText: Schema.String,
  fieldCount: Schema.Int,
  createdAt: IsoDateTimeString,
  updatedAt: IsoDateTimeString,
}) {}

export class TemplateRecord extends Schema.Class<TemplateRecord>("TemplateRecord")({
  id: TemplateId,
  name: NonEmptyTrimmedString,
  document: DocumentEnvelope,
  revision: DocumentRevision,
  createdAt: IsoDateTimeString,
  updatedAt: IsoDateTimeString,
}) {}

export class TemplateCreateInput extends Schema.Class<TemplateCreateInput>("TemplateCreateInput")({
  name: NonEmptyTrimmedString,
  document: DocumentEnvelope,
}) {}

export class TemplateUpdateInput extends Schema.Class<TemplateUpdateInput>("TemplateUpdateInput")({
  id: TemplateId,
  expectedRevision: DocumentRevision,
  name: NonEmptyTrimmedString,
  document: DocumentEnvelope,
}) {}

export class TemplateSaveAck extends Schema.Class<TemplateSaveAck>("TemplateSaveAck")({
  id: TemplateId,
  revision: DocumentRevision,
  updatedAt: IsoDateTimeString,
  previewText: Schema.String,
  fieldCount: Schema.Int,
}) {}

export class FirPlaceholderValue extends Schema.Class<FirPlaceholderValue>("FirPlaceholderValue")({
  firId: FirId,
  placeholderId: PlaceholderId,
  value: Schema.String,
  updatedAt: IsoDateTimeString,
}) {}

export class FirPlaceholderValueUpsertInput extends Schema.Class<FirPlaceholderValueUpsertInput>(
  "FirPlaceholderValueUpsertInput",
)({
  firId: FirId,
  placeholderId: PlaceholderId,
  value: Schema.String,
}) {}

export class FirPlaceholderValueRemoveInput extends Schema.Class<FirPlaceholderValueRemoveInput>(
  "FirPlaceholderValueRemoveInput",
)({
  firId: FirId,
  placeholderId: PlaceholderId,
}) {}

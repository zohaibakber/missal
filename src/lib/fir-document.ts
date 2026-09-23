import { Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";
import {
  overrideValuesFrom,
  type FieldDisplayMode,
  type FieldPresentationContext,
} from "#/lib/field";
import { FirRecord } from "#/lib/fir";
import { DocumentRevision, FirDocumentId, FirId, TemplateId } from "#/lib/ids";
import { indexPlaceholders, Placeholder } from "#/lib/placeholder";
import { IsoDateTimeString, NonEmptyTrimmedString } from "#/lib/schema";
import { FirPlaceholderValue } from "#/lib/templates";

export class FirDocumentSummary extends Schema.Class<FirDocumentSummary>("FirDocumentSummary")({
  id: FirDocumentId,
  firId: FirId,
  templateId: TemplateId,
  sourceTemplateRevision: DocumentRevision,
  title: NonEmptyTrimmedString,
  revision: DocumentRevision,
  position: Schema.Int.pipe(Schema.check(Schema.isGreaterThanOrEqualTo(0))),
  previewText: Schema.String,
  fieldCount: Schema.Int,
  createdAt: IsoDateTimeString,
  updatedAt: IsoDateTimeString,
}) {}

export class FirDocumentRecord extends Schema.Class<FirDocumentRecord>("FirDocumentRecord")({
  id: FirDocumentId,
  firId: FirId,
  templateId: TemplateId,
  sourceTemplateRevision: DocumentRevision,
  title: NonEmptyTrimmedString,
  document: DocumentEnvelope,
  revision: DocumentRevision,
  position: Schema.Int.pipe(Schema.check(Schema.isGreaterThanOrEqualTo(0))),
  createdAt: IsoDateTimeString,
  updatedAt: IsoDateTimeString,
}) {}

export class FirDocumentSaveAck extends Schema.Class<FirDocumentSaveAck>("FirDocumentSaveAck")({
  id: FirDocumentId,
  revision: DocumentRevision,
  updatedAt: IsoDateTimeString,
  previewText: Schema.String,
  fieldCount: Schema.Int,
}) {}

export class FirDocumentSaveInput extends Schema.Class<FirDocumentSaveInput>(
  "FirDocumentSaveInput",
)({
  id: FirDocumentId,
  expectedRevision: DocumentRevision,
  document: DocumentEnvelope,
}) {}

export class AddFirTemplatesInput extends Schema.Class<AddFirTemplatesInput>(
  "AddFirTemplatesInput",
)({
  firId: FirId,
  templateIds: Schema.Array(TemplateId),
}) {}

export class ReorderFirDocumentsInput extends Schema.Class<ReorderFirDocumentsInput>(
  "ReorderFirDocumentsInput",
)({
  firId: FirId,
  documentIds: Schema.Array(FirDocumentId),
}) {}

export class FirValueContext extends Schema.Class<FirValueContext>("FirValueContext")({
  fir: FirRecord,
  catalog: Schema.Array(Placeholder),
  overrides: Schema.Array(FirPlaceholderValue),
  sharedSettings: Schema.Record(Schema.String, Schema.String),
}) {
  toPresentation(displayMode: FieldDisplayMode): FieldPresentationContext {
    return {
      catalog: indexPlaceholders(this.catalog),
      displayMode,
      fir: this.fir,
      overrideValues: overrideValuesFrom(this.overrides),
      sharedSettings: this.sharedSettings,
    };
  }
}

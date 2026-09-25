import { Context, type Effect } from "effect";
import {
  AddFirTemplatesInput,
  FirDocumentRecord,
  FirDocumentSaveAck,
  FirDocumentSaveInput,
  FirDocumentSummary,
  ReorderFirDocumentsInput,
} from "#/lib/fir-document";
import { FirDocumentId, FirId } from "#/lib/ids";
import type { RepositoryError } from "#/lib/storage-errors";

export class FirDocumentRepository extends Context.Service<
  FirDocumentRepository,
  {
    readonly listForFir: (
      firId: FirId,
    ) => Effect.Effect<readonly FirDocumentSummary[], RepositoryError>;
    readonly get: (id: FirDocumentId) => Effect.Effect<FirDocumentRecord, RepositoryError>;
    /** Several documents in one round trip, in the order asked for. */
    readonly getMany: (
      ids: readonly FirDocumentId[],
    ) => Effect.Effect<readonly FirDocumentRecord[], RepositoryError>;
    readonly addTemplates: (
      input: AddFirTemplatesInput,
    ) => Effect.Effect<readonly FirDocumentSummary[], RepositoryError>;
    readonly save: (
      input: FirDocumentSaveInput,
    ) => Effect.Effect<FirDocumentSaveAck, RepositoryError>;
    readonly reorder: (
      input: ReorderFirDocumentsInput,
    ) => Effect.Effect<readonly FirDocumentSummary[], RepositoryError>;
    readonly remove: (id: FirDocumentId) => Effect.Effect<void, RepositoryError>;
  }
>()("missal/FirDocumentRepository") {}

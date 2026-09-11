import { Context, type Effect } from "effect";
import {
  FirCreateInput,
  FirDocumentUpdateInput,
  FirId,
  FirRecord,
  FirUpdateInput,
} from "#/lib/fir";
import type { RepositoryError } from "#/lib/storage-errors";

export class FirRepository extends Context.Service<
  FirRepository,
  {
    readonly list: Effect.Effect<readonly FirRecord[], RepositoryError>;
    readonly get: (id: FirId) => Effect.Effect<FirRecord, RepositoryError>;
    readonly create: (input: FirCreateInput) => Effect.Effect<FirRecord, RepositoryError>;
    readonly update: (input: FirUpdateInput) => Effect.Effect<FirRecord, RepositoryError>;
    readonly updateDocument: (
      input: FirDocumentUpdateInput,
    ) => Effect.Effect<FirRecord, RepositoryError>;
    readonly remove: (id: FirId) => Effect.Effect<void, RepositoryError>;
  }
>()("missal/FirRepository") {}

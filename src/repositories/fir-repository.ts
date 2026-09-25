import { Context, type Effect } from "effect";
import { FirCreateInput, FirId, FirRecord, FirSummary, FirUpdateInput } from "#/lib/fir";
import { FirValueContext } from "#/lib/fir-document";
import type { RepositoryError } from "#/lib/storage-errors";

export class FirRepository extends Context.Service<
  FirRepository,
  {
    readonly list: Effect.Effect<readonly FirSummary[], RepositoryError>;
    readonly get: (id: FirId) => Effect.Effect<FirRecord, RepositoryError>;
    readonly create: (input: FirCreateInput) => Effect.Effect<FirRecord, RepositoryError>;
    readonly update: (input: FirUpdateInput) => Effect.Effect<FirRecord, RepositoryError>;
    readonly remove: (id: FirId) => Effect.Effect<void, RepositoryError>;
    readonly getValueContext: (id: FirId) => Effect.Effect<FirValueContext, RepositoryError>;
  }
>()("missal/FirRepository") {}

import { Context, type Effect } from "effect";
import { FirId } from "#/lib/ids";
import {
  FirPlaceholderValue,
  FirPlaceholderValueRemoveInput,
  FirPlaceholderValueUpsertInput,
} from "#/lib/templates";
import type { RepositoryError } from "#/lib/storage-errors";

export class FirPlaceholderValueRepository extends Context.Service<
  FirPlaceholderValueRepository,
  {
    readonly listForFir: (
      firId: FirId,
    ) => Effect.Effect<readonly FirPlaceholderValue[], RepositoryError>;
    readonly upsert: (
      input: FirPlaceholderValueUpsertInput,
    ) => Effect.Effect<FirPlaceholderValue, RepositoryError>;
    readonly remove: (
      input: FirPlaceholderValueRemoveInput,
    ) => Effect.Effect<void, RepositoryError>;
  }
>()("missal/FirPlaceholderValueRepository") {}

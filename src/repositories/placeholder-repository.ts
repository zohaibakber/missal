import { Context, type Effect } from "effect";
import {
  Placeholder,
  PlaceholderCreateInput,
  PlaceholderId,
  PlaceholderUpdateInput,
} from "#/lib/placeholder";
import type { RepositoryError } from "#/lib/storage-errors";

export class PlaceholderRepository extends Context.Service<
  PlaceholderRepository,
  {
    readonly list: Effect.Effect<readonly Placeholder[], RepositoryError>;
    readonly create: (input: PlaceholderCreateInput) => Effect.Effect<Placeholder, RepositoryError>;
    readonly update: (input: PlaceholderUpdateInput) => Effect.Effect<Placeholder, RepositoryError>;
    readonly remove: (id: PlaceholderId) => Effect.Effect<void, RepositoryError>;
  }
>()("missal/PlaceholderRepository") {}

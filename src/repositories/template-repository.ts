import { Context, type Effect } from "effect";
import {
  TemplateCreateInput,
  TemplateId,
  TemplateRecord,
  TemplateUpdateInput,
} from "#/lib/templates";
import type { RepositoryError } from "#/lib/storage-errors";

export class TemplateRepository extends Context.Service<
  TemplateRepository,
  {
    readonly list: Effect.Effect<readonly TemplateRecord[], RepositoryError>;
    readonly get: (id: TemplateId) => Effect.Effect<TemplateRecord, RepositoryError>;
    readonly create: (input: TemplateCreateInput) => Effect.Effect<TemplateRecord, RepositoryError>;
    readonly update: (input: TemplateUpdateInput) => Effect.Effect<TemplateRecord, RepositoryError>;
    readonly remove: (id: TemplateId) => Effect.Effect<void, RepositoryError>;
  }
>()("missal/TemplateRepository") {}

import { Context, type Effect } from "effect";
import type { RepositoryError } from "#/lib/storage-errors";
import {
  TemplateCreateInput,
  TemplateId,
  TemplateRecord,
  TemplateSaveAck,
  TemplateSummary,
  TemplateUpdateInput,
} from "#/lib/templates";

export class TemplateRepository extends Context.Service<
  TemplateRepository,
  {
    readonly list: Effect.Effect<readonly TemplateSummary[], RepositoryError>;
    readonly search: (query: string) => Effect.Effect<readonly TemplateSummary[], RepositoryError>;
    readonly get: (id: TemplateId) => Effect.Effect<TemplateRecord, RepositoryError>;
    readonly create: (
      input: TemplateCreateInput,
    ) => Effect.Effect<TemplateSummary, RepositoryError>;
    readonly save: (input: TemplateUpdateInput) => Effect.Effect<TemplateSaveAck, RepositoryError>;
    readonly remove: (id: TemplateId) => Effect.Effect<void, RepositoryError>;
  }
>()("missal/TemplateRepository") {}

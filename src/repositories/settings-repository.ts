import { Context, type Effect } from "effect";
import type { AppSettings, FieldMarkers } from "#/lib/settings";
import type { RepositoryError } from "#/lib/storage-errors";

export class SettingsRepository extends Context.Service<
  SettingsRepository,
  {
    readonly get: Effect.Effect<AppSettings, RepositoryError>;
    readonly save: (
      sharedPlaceholders: Readonly<Record<string, string>>,
    ) => Effect.Effect<AppSettings, RepositoryError>;
    readonly saveFieldMarkers: (
      fieldMarkers: FieldMarkers,
    ) => Effect.Effect<AppSettings, RepositoryError>;
  }
>()("missal/SettingsRepository") {}

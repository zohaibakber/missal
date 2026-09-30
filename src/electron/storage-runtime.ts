import { Cause, Effect, Layer, ManagedRuntime, Ref } from "effect";
import {
  syncBundledTemplates,
  TemplatePackStatusStore,
  TemplatePackStatusStoreLive,
} from "#/electron/bundled-templates";
import { DatabasePath, MigrationsFolder } from "#/electron/database";
import { publishFreshProfile } from "#/electron/fresh-profile";
import { ElectronDatabaseLive } from "#/electron/repositories";
import type { StorageWorkerConfig } from "#/electron/storage-rpc";
import { TemplatePackPartialFailure, TemplatePackSynchronizing } from "#/lib/bundled-templates";

export function storageLayer(config: StorageWorkerConfig) {
  // Without a seed the database opens empty and the background installer fills it.
  return Layer.unwrap(
    Effect.tryPromise(() => publishFreshProfile(config)).pipe(
      Effect.catch((error) => Effect.logWarning("Bundled seed database was not used", error)),
      Effect.as(openStorageLayer(config)),
    ),
  );
}

function openStorageLayer(config: StorageWorkerConfig) {
  const database = ElectronDatabaseLive.pipe(
    Layer.provide(Layer.succeed(DatabasePath, config.databasePath)),
    Layer.provide(Layer.succeed(MigrationsFolder, config.migrationsFolder)),
    Layer.provideMerge(TemplatePackStatusStoreLive),
  );
  const folder = config.bundledTemplatesFolder;
  if (folder === undefined) return database;

  // Runs after migrations without holding up the first reads; a broken pack must not keep the
  // user's own data from opening.
  const bundled = Layer.effectDiscard(
    Effect.gen(function* () {
      const status = yield* TemplatePackStatusStore;
      yield* Ref.set(status, TemplatePackSynchronizing.make({ done: 0, total: 0 }));
      yield* Effect.forkScoped(
        syncBundledTemplates(folder).pipe(
          Effect.catchCause((cause) =>
            Cause.hasInterruptsOnly(cause)
              ? Effect.failCause(cause)
              : Effect.logWarning("Bundled templates were not installed", cause).pipe(
                  Effect.andThen(
                    Ref.set(
                      status,
                      TemplatePackPartialFailure.make({
                        message: "Bundled templates were not installed",
                      }),
                    ),
                  ),
                ),
          ),
        ),
      );
    }),
  );
  return Layer.provideMerge(bundled, database);
}

export function makeStorageRuntime(config: StorageWorkerConfig) {
  return ManagedRuntime.make(storageLayer(config));
}

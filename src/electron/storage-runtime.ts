import { Effect, Layer, ManagedRuntime } from "effect";
import { syncBundledTemplates } from "#/electron/bundled-templates";
import { DatabasePath, MigrationsFolder } from "#/electron/database";
import { ElectronDatabaseLive } from "#/electron/repositories";
import type { StorageWorkerConfig } from "#/electron/storage-rpc";

export function storageLayer(config: StorageWorkerConfig) {
  const database = ElectronDatabaseLive.pipe(
    Layer.provide(Layer.succeed(DatabasePath, config.databasePath)),
    Layer.provide(Layer.succeed(MigrationsFolder, config.migrationsFolder)),
  );
  const folder = config.bundledTemplatesFolder;
  if (folder === undefined) return database;
  // A broken pack must not keep the user's own data from opening.
  const bundled = Layer.effectDiscard(
    syncBundledTemplates(folder).pipe(
      Effect.catchCause((cause) =>
        Effect.logWarning("Bundled templates were not installed", cause),
      ),
    ),
  );
  return Layer.provideMerge(bundled, database);
}

export function makeStorageRuntime(config: StorageWorkerConfig) {
  return ManagedRuntime.make(storageLayer(config));
}

import { Layer, ManagedRuntime } from "effect";
import { DatabasePath, MigrationsFolder } from "#/electron/database";
import { ElectronDatabaseLive } from "#/electron/repositories";
import type { StorageWorkerConfig } from "#/electron/storage-rpc";

export function storageLayer(config: StorageWorkerConfig) {
  return ElectronDatabaseLive.pipe(
    Layer.provide(Layer.succeed(DatabasePath, config.databasePath)),
    Layer.provide(Layer.succeed(MigrationsFolder, config.migrationsFolder)),
  );
}

export function makeStorageRuntime(config: StorageWorkerConfig) {
  return ManagedRuntime.make(storageLayer(config));
}

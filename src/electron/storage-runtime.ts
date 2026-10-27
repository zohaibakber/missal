import { Layer } from "effect";
import { DatabasePath, MigrationsFolder } from "#/electron/database";
import { ElectronDatabaseLive } from "#/electron/repositories";
import type { StorageWorkerConfig } from "#/electron/storage-control-rpc";

export const storageLayer = (config: StorageWorkerConfig) =>
  ElectronDatabaseLive.pipe(
    Layer.provide(Layer.succeed(DatabasePath, config.databasePath)),
    Layer.provide(Layer.succeed(MigrationsFolder, config.migrationsFolder)),
  );

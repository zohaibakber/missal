import { Layer, ManagedRuntime } from "effect";
import path from "node:path";
import { DatabasePath, MigrationsFolder } from "#/electron/database";
import { ElectronDatabaseLive } from "#/electron/repositories";

export function resolveDatabasePath(userDataPath: string) {
  return path.join(userDataPath, "missal.sqlite");
}

export function resolveMigrationsFolder(options: {
  packaged: boolean;
  resourcesPath: string;
  cwd: string;
}) {
  return options.packaged
    ? path.join(options.resourcesPath, "drizzle")
    : path.join(options.cwd, "drizzle");
}

export function makeElectronMainRuntime(options: {
  databasePath: string;
  migrationsFolder: string;
}) {
  const layer = ElectronDatabaseLive.pipe(
    Layer.provide(Layer.succeed(DatabasePath, options.databasePath)),
    Layer.provide(Layer.succeed(MigrationsFolder, options.migrationsFolder)),
  );

  return ManagedRuntime.make(layer);
}

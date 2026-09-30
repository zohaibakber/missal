import { Cause, Effect, FiberHandle, Layer, ManagedRuntime, Ref } from "effect";
import {
  BundledPackFiles,
  BundledPackFilesLive,
  syncBundledTemplates,
  TemplatePackStatusStore,
  TemplatePackStatusStoreLive,
} from "#/electron/bundled-templates";
import { DatabasePath, MigrationsFolder } from "#/electron/database";
import { ElectronDatabaseLive } from "#/electron/repositories";
import type { StorageWorkerConfig } from "#/electron/storage-rpc";
import { TemplatePackPartialFailure, TemplatePackSynchronizing } from "#/lib/bundled-templates";
import type { StorageError } from "#/lib/storage-errors";

export type TemplatePackInstallerOptions = {
  readonly readManifest?: (folder: string) => Effect.Effect<Uint8Array, StorageError>;
  readonly readBody?: (folder: string, bodyHash: string) => Effect.Effect<Uint8Array, StorageError>;
  readonly beforeBatch?: (batchIndex: number) => Effect.Effect<void>;
};

function packFiles(installer?: TemplatePackInstallerOptions) {
  if (installer?.readManifest === undefined && installer?.readBody === undefined) {
    return BundledPackFilesLive;
  }
  const defaults = BundledPackFilesLive;
  return Layer.effect(
    BundledPackFiles,
    Effect.gen(function* () {
      const files = yield* BundledPackFiles;
      return BundledPackFiles.of({
        readBody: installer.readBody ?? files.readBody,
        readManifest: installer.readManifest ?? files.readManifest,
      });
    }),
  ).pipe(Layer.provide(defaults));
}

export function storageLayer(
  config: StorageWorkerConfig,
  installer?: TemplatePackInstallerOptions,
) {
  const database = ElectronDatabaseLive.pipe(
    Layer.provide(Layer.succeed(DatabasePath, config.databasePath)),
    Layer.provide(Layer.succeed(MigrationsFolder, config.migrationsFolder)),
    Layer.provideMerge(TemplatePackStatusStoreLive),
  );
  const folder = config.bundledTemplatesFolder;
  if (folder === undefined) return database;

  const beforeBatch = installer?.beforeBatch ?? (() => Effect.void);
  const job = Layer.effectDiscard(
    Effect.gen(function* () {
      const status = yield* TemplatePackStatusStore;
      const handle = yield* FiberHandle.make<void, unknown>();
      yield* Ref.set(status, TemplatePackSynchronizing.make({ done: 0, total: 0 }));
      yield* FiberHandle.run(
        handle,
        syncBundledTemplates(folder, beforeBatch).pipe(
          Effect.catchCause((cause) => {
            if (Cause.hasInterruptsOnly(cause)) return Effect.failCause(cause);
            return Effect.logWarning("Bundled templates were not installed", cause).pipe(
              Effect.andThen(
                Ref.set(
                  status,
                  TemplatePackPartialFailure.make({
                    message: "Bundled templates were not installed",
                  }),
                ),
              ),
            );
          }),
        ),
        { onlyIfMissing: true },
      );
    }),
  ).pipe(Layer.provide(packFiles(installer)));

  return job.pipe(Layer.provideMerge(database));
}

export function makeStorageRuntime(
  config: StorageWorkerConfig,
  installer?: TemplatePackInstallerOptions,
) {
  return ManagedRuntime.make(storageLayer(config, installer));
}

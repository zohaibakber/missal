import * as NodeWorker from "@effect/platform-node/NodeWorker";
import { Context, Effect, Layer, ManagedRuntime } from "effect";
import { RpcClient, RpcWorker } from "effect/unstable/rpc";
import type { RpcClientError } from "effect/unstable/rpc/RpcClientError";
import path from "node:path";
import { Worker } from "node:worker_threads";
import { StorageRpcs, StorageWorkerConfig } from "#/electron/storage-rpc";

export class StorageWorker extends Context.Service<
  StorageWorker,
  RpcClient.FromGroup<typeof StorageRpcs, RpcClientError>
>()("missal/StorageWorker") {}

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

export function makeStorageWorkerRuntime(options: {
  workerPath: string;
  config: StorageWorkerConfig;
}) {
  const layer = Layer.effect(StorageWorker, RpcClient.make(StorageRpcs)).pipe(
    Layer.provide(RpcClient.layerProtocolWorker({ size: 1 })),
    Layer.provide(NodeWorker.layer(() => new Worker(options.workerPath))),
    Layer.provide(
      RpcWorker.layerInitialMessage(StorageWorkerConfig, Effect.succeed(options.config)),
    ),
  );

  return ManagedRuntime.make(layer);
}

export type StorageHost = {
  readonly ready: Promise<void>;
  readonly request: (payload: string) => Promise<string>;
  readonly dispose: () => Promise<void>;
};

export function startStorageWorker(options: {
  workerPath: string;
  config: StorageWorkerConfig;
}): StorageHost {
  const runtime = makeStorageWorkerRuntime(options);
  const ready = runtime.runPromise(
    Effect.flatMap(StorageWorker, (worker) => worker["Storage.open"]()),
  );

  return {
    ready,
    request: async (payload) => {
      await ready;
      return runtime.runPromise(
        Effect.flatMap(StorageWorker, (worker) => worker["Storage.request"]({ payload })),
      );
    },
    dispose: () => runtime.dispose(),
  };
}

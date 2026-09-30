import * as NodeWorker from "@effect/platform-node/NodeWorker";
import { Buffer } from "node:buffer";
import path from "node:path";
import { Worker } from "node:worker_threads";
import { Cause, Context, Effect, Exit, Layer, ManagedRuntime, Semaphore } from "effect";
import { RpcClient, RpcWorker } from "effect/rpc";
import { RpcClientError } from "effect/rpc/RpcClientError";
import { encodeStorageResponse } from "#/electron/storage-contract";
import {
  STORAGE_MAX_PENDING,
  STORAGE_MAX_REQUEST_BYTES,
  STORAGE_MAX_RETAINED_BYTES,
  STORAGE_READINESS_MS,
  STORAGE_SHUTDOWN_MS,
  storageOperationPolicy,
} from "#/electron/storage-limits";
import { StorageRpcs, StorageWorkerConfig } from "#/electron/storage-rpc";
import {
  StorageBusy,
  StorageError,
  StorageUnavailable,
  StorageUnknownOutcome,
  type RepositoryError,
} from "#/lib/storage-errors";

export class StorageWorker extends Context.Service<
  StorageWorker,
  RpcClient.FromGroup<typeof StorageRpcs, RpcClientError>
>()("missal/StorageWorker") {}

export function resolveDatabasePath(userDataPath: string) {
  return path.join(userDataPath, "missal.sqlite");
}

/** A folder shipped with `extraResource`, or the repository's copy in development. */
export function resolveResourceFolder(
  name: string,
  options: { packaged: boolean; resourcesPath: string; cwd: string },
) {
  return path.join(options.packaged ? options.resourcesPath : options.cwd, name);
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

const failure = (error: RepositoryError) => encodeStorageResponse({ _tag: "Failure", error });

const unknownOutcome = (operation: string) =>
  failure(
    new StorageUnknownOutcome({
      message: "Storage did not confirm this change. It may have finished, so it was not repeated.",
      operation,
    }),
  );

export function startStorageWorker(options: {
  workerPath: string;
  config: StorageWorkerConfig;
}): StorageHost {
  // RpcClient.makeProtocolWorker already respawns a crashed worker, so readiness only
  // observes Storage.open instead of supervising the worker a second time.
  const runtime = makeStorageWorkerRuntime(options);
  const permit = Semaphore.makeUnsafe(1);
  let health: "starting" | "ready" | "unavailable" | "stopping" = "starting";
  let pending = 0;
  let retained = 0;
  let disposal: Promise<void> | undefined;

  const ready = runtime.runPromise(
    Effect.flatMap(StorageWorker, (worker) => worker["Storage.open"]()).pipe(
      Effect.timeoutOrElse({
        duration: STORAGE_READINESS_MS,
        orElse: () =>
          Effect.fail(
            new StorageUnavailable({
              message: "Storage did not become ready in time",
              operation: "storage.init",
            }),
          ),
      }),
    ),
  );
  ready.then(
    () => {
      if (health === "starting") health = "ready";
    },
    () => {
      if (health === "starting") health = "unavailable";
    },
  );

  const unavailable = (operation: string) =>
    failure(
      new StorageUnavailable({
        message: health === "stopping" ? "Storage is shutting down." : "Storage is not available.",
        operation,
      }),
    );

  const send = async (payload: string, policy: ReturnType<typeof storageOperationPolicy>) => {
    await ready.catch(() => undefined);
    if (health !== "ready") return unavailable(policy.operation);

    let sent = false;
    const exit = await runtime.runPromiseExit(
      permit.withPermit(
        Effect.flatMap(StorageWorker, (worker) => {
          sent = true;
          return worker["Storage.request"]({ payload });
        }).pipe(
          Effect.timeoutOrElse({
            duration: policy.deadlineMs,
            orElse: () =>
              Effect.succeed(
                policy.write
                  ? unknownOutcome(policy.operation)
                  : failure(
                      new StorageError({
                        message: "Storage took too long to respond.",
                        operation: policy.operation,
                      }),
                    ),
              ),
          }),
        ),
      ),
    );
    if (Exit.isSuccess(exit)) return exit.value;
    if (policy.write && sent) return unknownOutcome(policy.operation);
    if (Cause.hasInterruptsOnly(exit.cause)) return unavailable(policy.operation);
    console.error("Missal storage defect", exit.cause);
    return failure(
      new StorageError({ message: "Storage operation failed", operation: policy.operation }),
    );
  };

  return {
    ready,
    async request(payload) {
      const policy = storageOperationPolicy(payload);
      const bytes = Buffer.byteLength(payload, "utf8");
      if (bytes > STORAGE_MAX_REQUEST_BYTES) {
        return failure(
          new StorageError({
            message: "This document is too large to store.",
            operation: policy.operation,
          }),
        );
      }
      if (health === "unavailable" || health === "stopping") return unavailable(policy.operation);
      if (pending >= STORAGE_MAX_PENDING || retained + bytes > STORAGE_MAX_RETAINED_BYTES) {
        return failure(
          new StorageBusy({
            message: "Storage is busy. Wait a moment and try again.",
            operation: policy.operation,
          }),
        );
      }

      pending += 1;
      retained += bytes;
      try {
        return await send(payload, policy);
      } finally {
        pending -= 1;
        retained -= bytes;
      }
    },
    dispose() {
      health = "stopping";
      // Closing the runtime scope is uninterruptible, so an Effect timeout would wait for it anyway.
      disposal ??= new Promise<void>((resolve) => {
        const timer = setTimeout(resolve, STORAGE_SHUTDOWN_MS);
        const done = () => {
          clearTimeout(timer);
          resolve();
        };
        runtime.dispose().then(done, done);
      });
      return disposal;
    },
  };
}

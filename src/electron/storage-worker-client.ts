import * as NodeWorker from "@effect/platform-node/NodeWorker";
import { Buffer } from "node:buffer";
import path from "node:path";
import { Worker } from "node:worker_threads";
import {
  Cause,
  Context,
  Deferred,
  Effect,
  Exit,
  Layer,
  ManagedRuntime,
  Queue,
  Semaphore,
} from "effect";
import { RpcClient, RpcWorker } from "effect/rpc";
import { RpcClientError } from "effect/rpc/RpcClientError";
import { encodeStorageResponse } from "#/electron/storage-contract";
import {
  STORAGE_MAX_ACTIVE,
  STORAGE_MAX_QUEUED,
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

export type StorageHealth = "starting" | "ready" | "unavailable" | "stopping";

export type StorageHost = {
  readonly health: () => StorageHealth;
  readonly ready: Promise<void>;
  readonly request: (payload: string) => Promise<string>;
  readonly dispose: () => Promise<void>;
};

const failure = (error: RepositoryError) => encodeStorageResponse({ _tag: "Failure", error });

const busyBody = (operation: string) =>
  failure(
    new StorageBusy({
      message: "Storage is busy. Wait a moment and try again.",
      operation,
    }),
  );

const unavailableBody = (operation: string, message = "Storage is not available.") =>
  failure(new StorageUnavailable({ message, operation }));

const unknownBody = (operation: string) =>
  failure(
    new StorageUnknownOutcome({
      message: "Storage did not confirm this change. It may have finished, so it was not repeated.",
      operation,
    }),
  );

const readTimeoutBody = (operation: string) =>
  failure(
    new StorageError({
      message: "Storage took too long to respond.",
      operation,
    }),
  );

const failedBody = (operation: string) =>
  failure(
    new StorageError({
      message: "Storage operation failed",
      operation,
    }),
  );

const tooLargeBody = () =>
  failure(
    new StorageError({
      message: "This document is too large to store.",
      operation: "storage.request",
    }),
  );

type Job = {
  readonly payload: string;
  readonly bytes: number;
  readonly operation: string;
  readonly write: boolean;
  readonly deadlineMs: number;
  started: boolean;
  settled: boolean;
  readonly resolve: (body: string) => void;
};

export function startStorageWorker(options: {
  workerPath: string;
  config: StorageWorkerConfig;
}): StorageHost {
  // RpcClient.makeProtocolWorker already respawns a crashed worker on a 1s schedule.
  // Readiness only observes Storage.open; it does not start another supervisor.
  const runtime = makeStorageWorkerRuntime(options);
  const queue = Effect.runSync(Queue.dropping<Job>(STORAGE_MAX_QUEUED));
  const permits = Semaphore.makeUnsafe(STORAGE_MAX_ACTIVE);
  const opened = Deferred.makeUnsafe<void, StorageError | RpcClientError>();
  const pending = new Set<Job>();
  const startedAt = Date.now();
  let healthState: StorageHealth = "starting";
  let openSettled = false;
  let retained = 0;
  let disposal: Promise<void> | undefined;
  const isStopping = () => healthState === "stopping";

  const settle = (job: Job, body: string) => {
    if (job.settled) return;
    job.settled = true;
    pending.delete(job);
    retained -= job.bytes;
    job.resolve(body);
  };

  const unfinishedBody = (job: Job) => {
    if (job.write && job.started) return unknownBody(job.operation);
    if (isStopping()) return unavailableBody(job.operation, "Storage is shutting down.");
    return failedBody(job.operation);
  };

  const waitForReady = Effect.gen(function* () {
    if (healthState === "ready") return true;
    if (healthState !== "starting") return false;
    const remaining = STORAGE_READINESS_MS - (Date.now() - startedAt);
    if (remaining <= 0) {
      healthState = "unavailable";
      return false;
    }
    return yield* Deferred.await(opened).pipe(
      Effect.timeoutOrElse({
        duration: remaining,
        orElse: () =>
          Effect.fail(
            new StorageError({
              message: "Storage did not become ready in time",
              operation: "storage.init",
            }),
          ),
      }),
      Effect.match({
        onFailure: () => {
          if (healthState === "starting") healthState = "unavailable";
          return false;
        },
        onSuccess: () => healthState === "ready",
      }),
    );
  });

  const runJob = (job: Job) =>
    Effect.gen(function* () {
      if (job.settled || isStopping()) {
        settle(job, isStopping() ? unfinishedBody(job) : unavailableBody(job.operation));
        return;
      }
      const open = yield* waitForReady;
      if (!open || healthState !== "ready") {
        settle(
          job,
          isStopping()
            ? unfinishedBody(job)
            : unavailableBody(
                job.operation,
                openSettled ? "Storage is not available." : "Storage did not become ready in time",
              ),
        );
        return;
      }
      const worker = yield* StorageWorker;
      job.started = true;
      const response = yield* worker["Storage.request"]({ payload: job.payload }).pipe(
        Effect.timeoutOrElse({
          duration: job.deadlineMs,
          orElse: () =>
            Effect.succeed(job.write ? unknownBody(job.operation) : readTimeoutBody(job.operation)),
        }),
        Effect.catchCause((cause) => {
          if (!Cause.hasInterruptsOnly(cause)) console.error("Missal storage defect", cause);
          return Effect.succeed(job.write ? unknownBody(job.operation) : failedBody(job.operation));
        }),
      );
      settle(job, response);
    }).pipe(
      Effect.ensuring(
        Effect.sync(() => {
          if (!job.settled) settle(job, unfinishedBody(job));
        }),
      ),
    );

  runtime.runFork(
    Effect.matchCauseEffect(
      Effect.flatMap(StorageWorker, (worker) => worker["Storage.open"]()),
      {
        onFailure: (cause) =>
          Effect.sync(() => {
            openSettled = true;
            if (!isStopping()) healthState = "unavailable";
            Deferred.doneUnsafe(opened, Effect.failCause(cause));
          }),
        onSuccess: () =>
          Effect.sync(() => {
            openSettled = true;
            if (!isStopping()) healthState = "ready";
            Deferred.doneUnsafe(opened, Effect.void);
          }),
      },
    ),
  );

  runtime.runFork(
    Effect.sleep(STORAGE_READINESS_MS).pipe(
      Effect.andThen(
        Effect.sync(() => {
          if (!openSettled && healthState === "starting") healthState = "unavailable";
        }),
      ),
    ),
  );

  runtime.runFork(
    Effect.forever(
      Queue.take(queue).pipe(
        Effect.flatMap((job) => permits.withPermit(runJob(job))),
        Effect.catchCause((cause) => {
          if (Cause.hasInterruptsOnly(cause)) return Effect.failCause(cause);
          console.error("Missal storage defect", cause);
          return Effect.void;
        }),
      ),
    ),
  );

  const ready = new Promise<void>((resolve, reject) => {
    runtime.runCallback(Deferred.await(opened), {
      onExit(exit) {
        if (Exit.isSuccess(exit) || isStopping()) {
          resolve();
          return;
        }
        const error = Cause.squash(exit.cause);
        reject(error instanceof Error ? error : new Error("Storage operation failed"));
      },
    });
  });

  return {
    health: () => healthState,
    ready,
    request(payload) {
      const bytes = Buffer.byteLength(payload, "utf8");
      if (bytes > STORAGE_MAX_REQUEST_BYTES) return Promise.resolve(tooLargeBody());
      const policy = storageOperationPolicy(payload);
      if (isStopping()) {
        return Promise.resolve(unavailableBody(policy.operation, "Storage is shutting down."));
      }
      if (healthState === "unavailable") return Promise.resolve(unavailableBody(policy.operation));
      if (retained + bytes > STORAGE_MAX_RETAINED_BYTES) {
        return Promise.resolve(busyBody(policy.operation));
      }

      let resolveJob!: (body: string) => void;
      const result = new Promise<string>((resolve) => {
        resolveJob = resolve;
      });
      const job: Job = {
        bytes,
        deadlineMs: policy.deadlineMs,
        operation: policy.operation,
        payload,
        resolve: resolveJob,
        settled: false,
        started: false,
        write: policy.write,
      };
      retained += bytes;
      pending.add(job);
      if (!Queue.offerUnsafe(queue, job)) {
        if (!job.settled) {
          pending.delete(job);
          retained -= job.bytes;
        }
        return Promise.resolve(isStopping() ? unfinishedBody(job) : busyBody(job.operation));
      }
      return result;
    },
    dispose() {
      healthState = "stopping";
      for (const job of pending) settle(job, unfinishedBody(job));
      Queue.shutdownUnsafe(queue);
      disposal ??= runtime.runPromise(
        runtime.disposeEffect.pipe(
          Effect.timeoutOrElse({
            duration: STORAGE_SHUTDOWN_MS,
            orElse: () => Effect.void,
          }),
        ),
      );
      return disposal;
    },
  };
}

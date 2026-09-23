import { ipcMain } from "electron";
import { Effect } from "effect";
import type { ManagedRuntime } from "effect/ManagedRuntime";
import { STORAGE_CHANNEL } from "#/electron/storage-channel";
import { encodeStorageResponse } from "#/electron/storage-contract";
import { StorageWorker } from "#/electron/storage-worker-client";
import { StorageError } from "#/lib/storage-errors";

export function registerStorageIpc(
  runtime: ManagedRuntime<StorageWorker, unknown>,
  ready: Promise<void>,
) {
  ipcMain.handle(STORAGE_CHANNEL, async (_event, payload: unknown) => {
    await ready;
    return runtime.runPromise(
      Effect.flatMap(StorageWorker, (worker) => worker["Storage.request"]({ payload })).pipe(
        Effect.catch((error) =>
          Effect.succeed(
            encodeStorageResponse({
              _tag: "Failure",
              error:
                error instanceof StorageError
                  ? error
                  : new StorageError({
                      message: "Storage operation failed",
                      operation: "storage.request",
                    }),
            }),
          ),
        ),
      ),
    );
  });
}

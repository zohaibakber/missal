import { ipcMain } from "electron";
import type { ManagedRuntime } from "effect/ManagedRuntime";
import { STORAGE_CHANNEL } from "#/electron/storage-contract";
import { dispatchStorageRequest } from "#/electron/storage-dispatch";
import type { AppRepositories } from "#/repositories/index";

export function registerStorageIpc(runtime: ManagedRuntime<AppRepositories, unknown>) {
  ipcMain.handle(STORAGE_CHANNEL, (_event, payload: unknown) =>
    runtime.runPromise(dispatchStorageRequest(payload)),
  );
}

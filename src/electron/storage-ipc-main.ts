import { ipcMain } from "electron";
import { STORAGE_CHANNEL } from "#/electron/storage-channel";
import type { StorageHost } from "#/electron/storage-worker-client";

/**
 * Registers the storage channel straight away; requests wait for the host, which loads after the
 * window so Effect and SQLite don't delay first paint. A rejection reaches the renderer's
 * repository as a StorageError.
 */
export function registerStorageIpc(host: Promise<StorageHost>) {
  ipcMain.handle(STORAGE_CHANNEL, async (_event, payload: unknown) => {
    if (typeof payload !== "string") {
      throw new Error("Invalid storage request");
    }
    return (await host).request(payload);
  });
}

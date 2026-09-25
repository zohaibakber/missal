import { ipcMain } from "electron";
import { STORAGE_CHANNEL } from "#/electron/storage-channel";
import type { StorageHost } from "#/electron/storage-worker-client";

export function registerStorageIpc(host: Promise<StorageHost>) {
  ipcMain.handle(STORAGE_CHANNEL, async (_event, payload: unknown) => {
    if (typeof payload !== "string") {
      throw new Error("Invalid storage request");
    }
    return (await host).request(payload);
  });
}

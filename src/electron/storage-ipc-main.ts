import { ipcMain } from "electron";
import { STORAGE_CHANNEL } from "#/electron/storage-channel";
import type { StorageHost } from "#/electron/storage-worker-client";

export function registerStorageIpc(host: Promise<StorageHost>, origin: string) {
  ipcMain.handle(STORAGE_CHANNEL, async (event, payload: unknown) => {
    const senderUrl = event.senderFrame?.url;
    if (!senderUrl || new URL(senderUrl).origin !== origin) {
      throw new Error("Storage request rejected");
    }
    if (typeof payload !== "string") {
      throw new Error("Invalid storage request");
    }
    return (await host).request(payload);
  });
}

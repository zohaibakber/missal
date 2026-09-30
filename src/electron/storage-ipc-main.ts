import { ipcMain } from "electron";
import { STORAGE_CHANNEL } from "#/electron/storage-channel";
import { STORAGE_READINESS_MS } from "#/electron/storage-limits";
import type { StorageHost } from "#/electron/storage-worker-client";

// Same JSON `encodeStorageResponse` produces for this failure. Inlined so startup does not load Schema.
const NOT_READY =
  '{"_tag":"Failure","error":{"_tag":"StorageUnavailable","operation":"storage.init","message":"Storage did not become ready in time"}}';

export function registerStorageIpc(host: Promise<StorageHost>, origin: string) {
  const deadlineAt = Date.now() + STORAGE_READINESS_MS;

  ipcMain.handle(STORAGE_CHANNEL, async (event, payload: unknown) => {
    const senderUrl = event.senderFrame?.url;
    if (!senderUrl || new URL(senderUrl).origin !== origin) {
      throw new Error("Storage request rejected");
    }
    if (typeof payload !== "string") {
      throw new Error("Invalid storage request");
    }

    const remaining = deadlineAt - Date.now();
    const readyHost = await new Promise<StorageHost | undefined>((resolve) => {
      const timer = setTimeout(() => resolve(undefined), Math.max(0, remaining));
      host.then(
        (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        () => {
          clearTimeout(timer);
          resolve(undefined);
        },
      );
    });
    if (!readyHost) return NOT_READY;
    return readyHost.request(payload);
  });
}

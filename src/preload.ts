import { contextBridge, ipcRenderer } from "electron";
import { DESKTOP_THEME_CHANNEL, type ElectronThemeApi } from "./desktop-window";
import { STORAGE_CHANNEL } from "./electron/storage-channel";

const electronTheme: ElectronThemeApi = {
  setSource: (source) => ipcRenderer.send(DESKTOP_THEME_CHANNEL, source),
};

const electronStorage = {
  request: (payload: unknown) => ipcRenderer.invoke(STORAGE_CHANNEL, payload),
};

contextBridge.exposeInMainWorld("electronTheme", electronTheme);
contextBridge.exposeInMainWorld("electronStorage", electronStorage);

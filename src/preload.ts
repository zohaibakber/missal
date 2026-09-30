import { contextBridge, ipcRenderer } from "electron";
import {
  DESKTOP_PRINT_CANCEL_CHANNEL,
  DESKTOP_PRINT_CHANNEL,
  DESKTOP_PRINT_PDF_CHANNEL,
  DESKTOP_THEME_CHANNEL,
  type ElectronPrintApi,
  type ElectronThemeApi,
} from "./desktop-window";
import { STORAGE_CHANNEL } from "./electron/storage-channel";

const electronTheme: ElectronThemeApi = {
  setSource: (source) => ipcRenderer.send(DESKTOP_THEME_CHANNEL, source),
};

const electronPrint: ElectronPrintApi = {
  renderPdf: (request) => ipcRenderer.invoke(DESKTOP_PRINT_PDF_CHANNEL, request),
  print: (request) => ipcRenderer.invoke(DESKTOP_PRINT_CHANNEL, request),
  cancelPreview: (request) => ipcRenderer.send(DESKTOP_PRINT_CANCEL_CHANNEL, request),
};

const electronStorage = {
  request: (payload: string): Promise<string> => ipcRenderer.invoke(STORAGE_CHANNEL, payload),
};

contextBridge.exposeInMainWorld("electronTheme", electronTheme);
contextBridge.exposeInMainWorld("electronPrint", electronPrint);
contextBridge.exposeInMainWorld("electronStorage", electronStorage);

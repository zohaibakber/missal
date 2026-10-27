import { contextBridge, ipcRenderer } from "electron";
import {
  DESKTOP_LAYOUT_CHANNEL,
  DESKTOP_PREFERENCES_CHANNEL,
  DESKTOP_PRINT_CHANNEL,
  DESKTOP_PRINT_PDF_CHANNEL,
  DESKTOP_THEME_CHANNEL,
  type DesktopPreferences,
  type ElectronPreferencesApi,
  type ElectronPrintApi,
} from "./desktop-window";
import { STORAGE_CONNECT_CHANNEL, STORAGE_PORT_CHANNEL } from "./electron/storage-channel";

const preferences: DesktopPreferences = ipcRenderer.sendSync(DESKTOP_PREFERENCES_CHANNEL);
const layouts = { ...preferences.layouts };

const electronPreferences: ElectronPreferencesApi = {
  theme: preferences.theme,
  setTheme: (theme) => ipcRenderer.send(DESKTOP_THEME_CHANNEL, theme),
  layoutStorage: {
    getItem: (key) => layouts[key] ?? null,
    setItem: (key, value) => {
      layouts[key] = value;
      ipcRenderer.send(DESKTOP_LAYOUT_CHANNEL, key, value);
    },
  },
};

const electronPrint: ElectronPrintApi = {
  renderPdf: (html) => ipcRenderer.invoke(DESKTOP_PRINT_PDF_CHANNEL, html),
  print: (html) => ipcRenderer.invoke(DESKTOP_PRINT_CHANNEL, html),
};

ipcRenderer.on(STORAGE_PORT_CHANNEL, (event) => {
  window.postMessage(STORAGE_PORT_CHANNEL, window.location.origin, event.ports);
});

contextBridge.exposeInMainWorld("electronPreferences", electronPreferences);
contextBridge.exposeInMainWorld("electronPrint", electronPrint);
contextBridge.exposeInMainWorld("electronStorage", {
  connect: () => ipcRenderer.send(STORAGE_CONNECT_CHANNEL),
});

/// <reference types="vite/client" />

import type { ElectronPreferencesApi, ElectronPrintApi } from "./desktop-window";

declare global {
  interface Window {
    electronPreferences: ElectronPreferencesApi;
    electronPrint: ElectronPrintApi;
    electronStorage: {
      connect: () => void;
    };
  }
}

export {};

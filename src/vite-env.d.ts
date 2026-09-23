/// <reference types="vite/client" />

import type { ElectronPrintApi, ElectronThemeApi } from "./desktop-window";

declare global {
  interface Window {
    electronTheme?: ElectronThemeApi;
    electronPrint?: ElectronPrintApi;
    electronStorage?: {
      request: (payload: unknown) => Promise<unknown>;
    };
  }
}

export {};

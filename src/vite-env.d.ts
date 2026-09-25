/// <reference types="vite/client" />

import type { ElectronPrintApi, ElectronThemeApi } from "./desktop-window";

declare global {
  interface Window {
    electronTheme?: ElectronThemeApi;
    electronPrint?: ElectronPrintApi;
    electronStorage?: {
      /** Sends one JSON-encoded storage request; resolves with the JSON response. */
      request: (payload: string) => Promise<string>;
    };
  }
}

export {};

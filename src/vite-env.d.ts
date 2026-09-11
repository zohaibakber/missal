/// <reference types="vite/client" />

import type { ElectronThemeApi } from "./desktop-window";

declare global {
  interface Window {
    electronTheme?: ElectronThemeApi;
    electronStorage?: {
      request: (payload: unknown) => Promise<unknown>;
    };
  }
}

export {};

export const DESKTOP_PREFERENCES_CHANNEL = "desktop:preferences";
export const DESKTOP_THEME_CHANNEL = "desktop:set-theme";

export type DesktopTheme = "dark" | "light" | "system";

export type DesktopPreferences = {
  readonly theme: DesktopTheme;
};

export type ElectronPreferencesApi = {
  readonly theme: DesktopTheme;
  readonly setTheme: (theme: DesktopTheme) => void;
};

export const DESKTOP_PRINT_PDF_CHANNEL = "desktop:print-pdf";
export const DESKTOP_PRINT_CHANNEL = "desktop:print";

export type PrintResult = {
  readonly printed: boolean;
  readonly failureReason?: string;
};

export type ElectronPrintApi = {
  renderPdf: (html: string) => Promise<Uint8Array>;
  print: (html: string) => Promise<PrintResult>;
};

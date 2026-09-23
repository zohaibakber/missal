export const DESKTOP_THEME_CHANNEL = "desktop:set-theme";

export type DesktopTheme = "dark" | "light" | "system";

export type ElectronThemeApi = {
  setSource: (source: DesktopTheme) => void;
};

export const DESKTOP_PRINT_PDF_CHANNEL = "desktop:print-pdf";

export type ElectronPrintApi = {
  /** Paginates a print packet exactly as the printer would and returns it as a PDF. */
  renderPdf: (html: string) => Promise<Uint8Array>;
};

export const DESKTOP_THEME_CHANNEL = "desktop:set-theme";

export type DesktopTheme = "dark" | "light" | "system";

export type ElectronThemeApi = {
  setSource: (source: DesktopTheme) => void;
};

export const DESKTOP_PRINT_PDF_CHANNEL = "desktop:print-pdf";
export const DESKTOP_PRINT_CHANNEL = "desktop:print";

export type PrintResult = {
  readonly printed: boolean;
  /** Chromium's reason when nothing printed, e.g. "Print job canceled". */
  readonly failureReason?: string;
};

export type ElectronPrintApi = {
  /** Paginates a print packet exactly as the printer would and returns it as a PDF. */
  renderPdf: (html: string) => Promise<Uint8Array>;
  /** Opens the system print dialog for a packet, reusing the layout made for its preview. */
  print: (html: string) => Promise<PrintResult>;
};

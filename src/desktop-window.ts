export const DESKTOP_THEME_CHANNEL = "desktop:set-theme";

export type DesktopTheme = "dark" | "light" | "system";

export type ElectronThemeApi = {
  setSource: (source: DesktopTheme) => void;
};

export const DESKTOP_PRINT_PDF_CHANNEL = "desktop:print-pdf";
export const DESKTOP_PRINT_CHANNEL = "desktop:print";
export const DESKTOP_PRINT_CANCEL_CHANNEL = "desktop:print-cancel";

export type PrintDisposition = "busy" | "timed-out" | "failed" | "cancelled";

export type PrintResult = {
  readonly printed: boolean;
  readonly failureReason?: string;
  readonly disposition?: PrintDisposition;
};

export type PrintRequest = {
  readonly ownerId: string;
  readonly requestId: string;
  readonly html: string;
};

export type PrintCancelRequest = {
  readonly ownerId: string;
  readonly requestId: string;
};

export type PdfRenderResult =
  | { readonly _tag: "Ok"; readonly pdf: Uint8Array }
  | { readonly _tag: "Unavailable"; readonly message: string }
  | { readonly _tag: "Ignored" };

export type ElectronPrintApi = {
  renderPdf: (request: PrintRequest) => Promise<PdfRenderResult>;
  print: (request: PrintRequest) => Promise<PrintResult>;
  cancelPreview: (request: PrintCancelRequest) => void;
};

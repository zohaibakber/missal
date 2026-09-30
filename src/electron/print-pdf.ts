import { BrowserWindow } from "electron";
import type { PdfRenderResult, PrintResult } from "#/desktop-window";
import {
  PRINT_IDLE_CLOSE_MS,
  createPrintService,
  type PrintJobRequest,
  type PrintJobResult,
  type PrintJobRunner,
  type PrintOwner,
} from "#/electron/print-jobs";

// A blank page served from the renderer's own origin, so the packet's relative font URL resolves
// exactly as it does in the app.
const PRINT_PAGE_PATH = "/print.html";

const writeDocument = (html: string) => `(() => {
  document.open();
  document.write(${JSON.stringify(html)});
  document.close();
  document.body.getBoundingClientRect();
  return true;
})()`;

const FONTS_SCRIPT = "document.fonts.ready.then(() => true)";

const IMAGES_SCRIPT = `Promise.all(Array.from(document.images, (image) =>
  image.complete
    ? null
    : new Promise((resolve) => {
        image.addEventListener("load", resolve, { once: true });
        image.addEventListener("error", resolve, { once: true });
      }),
)).then(() => true)`;

type PrintTarget = {
  readonly window: BrowserWindow;
  readonly loaded: Promise<void>;
  html?: string;
  settledHtml?: string;
};

const throwIfAborted = (signal: AbortSignal) => {
  if (signal.aborted) throw new Error("The print view closed.");
};

const createElectronPrintRunner = (): PrintJobRunner => {
  let target: PrintTarget | undefined;
  let idleTimer: NodeJS.Timeout | undefined;

  const clearIdle = () => {
    clearTimeout(idleTimer);
    idleTimer = undefined;
  };

  const resetWindow = () => {
    clearIdle();
    const current = target;
    target = undefined;
    if (current && !current.window.isDestroyed()) current.window.destroy();
  };

  const noteIdle = () => {
    clearIdle();
    if (!target || target.window.isDestroyed()) return;
    // The hidden window stays warm between previews: reopening one reuses its renderer
    // and the already decoded Urdu font. PRINT_IDLE_CLOSE_MS is the lifetime to compare.
    idleTimer = setTimeout(resetWindow, PRINT_IDLE_CLOSE_MS);
    idleTimer.unref();
  };

  const acquire = (origin: string) => {
    clearIdle();
    if (target && !target.window.isDestroyed()) return target;

    const window = new BrowserWindow({
      show: false,
      width: 900,
      height: 1200,
      webPreferences: { contextIsolation: true, nodeIntegration: false, sandbox: true },
    });
    const created: PrintTarget = {
      window,
      loaded: window.loadURL(new URL(PRINT_PAGE_PATH, origin).href).then(() => undefined),
    };
    window.once("closed", () => {
      if (target === created) target = undefined;
    });
    window.webContents.once("render-process-gone", () => {
      resetWindow();
    });
    target = created;
    return created;
  };

  const requireTarget = () => {
    if (!target || target.window.isDestroyed()) throw new Error("The print view closed.");
    return target;
  };

  const track = <T>(window: BrowserWindow, signal: AbortSignal, operation: Promise<T>) =>
    new Promise<T>((resolve, reject) => {
      let settled = false;
      const finish = (error?: Error, value?: T) => {
        if (settled) return;
        settled = true;
        signal.removeEventListener("abort", onAbort);
        if (!window.isDestroyed()) {
          window.removeListener("closed", onClosed);
          window.webContents.removeListener("render-process-gone", onGone);
        }
        if (error) reject(error);
        else resolve(value as T);
      };
      const onAbort = () => finish(new Error("The print view closed."));
      const onClosed = () => finish(new Error("The print view closed."));
      const onGone = () => finish(new Error("The print view crashed."));
      if (signal.aborted || window.isDestroyed()) {
        finish(new Error("The print view closed."));
        return;
      }
      signal.addEventListener("abort", onAbort, { once: true });
      window.once("closed", onClosed);
      window.webContents.once("render-process-gone", onGone);
      operation.then(
        (value) => finish(undefined, value),
        (error: unknown) =>
          finish(error instanceof Error ? error : new Error("The print view failed. Try again.")),
      );
    });

  const runPhase: PrintJobRunner["runPhase"] = async (phase, job, signal) => {
    throwIfAborted(signal);
    if (phase === "pageLoad") {
      const current = acquire(job.origin);
      await track(current.window, signal, current.loaded);
      return;
    }

    const current = requireTarget();
    if (
      (phase === "layout" || phase === "fonts" || phase === "images") &&
      current.settledHtml === job.html
    ) {
      return;
    }

    if (phase === "layout") {
      if (current.html === job.html) return;
      current.html = undefined;
      current.settledHtml = undefined;
      await track(
        current.window,
        signal,
        current.window.webContents.executeJavaScript(writeDocument(job.html)).then(() => {
          current.html = job.html;
        }),
      );
      return;
    }

    if (phase === "fonts") {
      await track(
        current.window,
        signal,
        current.window.webContents.executeJavaScript(FONTS_SCRIPT),
      );
      return;
    }

    if (phase === "images") {
      await track(
        current.window,
        signal,
        current.window.webContents.executeJavaScript(IMAGES_SCRIPT).then(() => {
          current.settledHtml = job.html;
        }),
      );
      return;
    }

    if (phase === "pdf") {
      return track(
        current.window,
        signal,
        current.window.webContents
          .printToPDF({ preferCSSPageSize: true, printBackground: true })
          .then((pdf) => new Uint8Array(pdf)),
      );
    }

    return track(
      current.window,
      signal,
      new Promise<PrintResult>((resolve, reject) => {
        if (current.window.isDestroyed()) {
          reject(new Error("The print view closed."));
          return;
        }
        current.window.webContents.print({ printBackground: true }, (printed, failureReason) => {
          resolve(printed ? { printed } : { printed, failureReason });
        });
      }),
    );
  };

  return { runPhase, resetWindow, noteIdle };
};

const runner = createElectronPrintRunner();
const service = createPrintService(runner);

const toPdfResult = (result: PrintJobResult): PdfRenderResult => {
  switch (result._tag) {
    case "Pdf":
      return { _tag: "Ok", pdf: result.pdf };
    case "Busy":
    case "TimedOut":
    case "Failed":
      return { _tag: "Unavailable", message: result.message };
    default:
      return { _tag: "Ignored" };
  }
};

const toPrintResult = (result: PrintJobResult): PrintResult => {
  switch (result._tag) {
    case "Printed":
      return { printed: result.printed, failureReason: result.failureReason };
    case "Busy":
      return { printed: false, failureReason: result.message, disposition: "busy" };
    case "TimedOut":
      return { printed: false, failureReason: result.message, disposition: "timed-out" };
    case "Failed":
      return { printed: false, failureReason: result.message, disposition: "failed" };
    case "Cancelled":
    case "Superseded":
      return { printed: false, disposition: "cancelled" };
    default:
      return { printed: false, failureReason: "Unable to print", disposition: "failed" };
  }
};

const submit = (kind: PrintJobRequest["kind"], request: Omit<PrintJobRequest, "kind">) =>
  service.submit({ ...request, kind });

export function renderPrintPdf(request: Omit<PrintJobRequest, "kind">) {
  return submit("preview", request).then(toPdfResult);
}

export function printPacket(request: Omit<PrintJobRequest, "kind">) {
  return submit("print", request).then(toPrintResult);
}

export function cancelPrintPreview(owner: PrintOwner, requestId: string) {
  service.cancelPreview(owner, requestId);
}

export function releasePrintOwner(webContentsId: number) {
  service.releaseWebContents(webContentsId);
}

export function closePrintWindow() {
  runner.resetWindow();
}

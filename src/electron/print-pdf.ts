import { BrowserWindow } from "electron";
import type { PrintResult } from "#/desktop-window";

// A blank page served from the renderer's own origin, so the packet's relative font URL resolves
// exactly as it does in the app.
const PRINT_PAGE_PATH = "/print.html";
// The hidden window stays warm between previews: reopening one reuses its renderer process and
// the already decoded Urdu font instead of starting both from scratch.
const IDLE_CLOSE_MS = 2 * 60_000;

const writePacket = (html: string) => `(async () => {
  document.open();
  document.write(${JSON.stringify(html)});
  document.close();
  document.body.getBoundingClientRect();
  await Promise.all([
    document.fonts.ready,
    ...Array.from(document.images, (image) =>
      image.complete
        ? null
        : new Promise((resolve) => {
            image.addEventListener("load", resolve, { once: true });
            image.addEventListener("error", resolve, { once: true });
          }),
    ),
  ]);
  return true;
})()`;

type PrintTarget = {
  readonly window: BrowserWindow;
  readonly loaded: Promise<void>;
  html?: string;
};

let target: PrintTarget | undefined;
let idleTimer: NodeJS.Timeout | undefined;
let queue: Promise<unknown> = Promise.resolve();

/** One window serves every packet, so a preview and a print never write over each other. */
function serialize<A>(task: () => Promise<A>) {
  const run = queue.then(task, task);
  queue = run.catch(() => undefined);
  return run;
}

function acquire(rendererOrigin: string) {
  clearTimeout(idleTimer);
  if (target && !target.window.isDestroyed()) return target;

  const window = new BrowserWindow({
    show: false,
    width: 900,
    height: 1200,
    webPreferences: { contextIsolation: true, nodeIntegration: false, sandbox: true },
  });
  const created: PrintTarget = {
    window,
    loaded: window.loadURL(new URL(PRINT_PAGE_PATH, rendererOrigin).href),
  };
  window.once("closed", () => {
    if (target === created) target = undefined;
  });
  window.webContents.once("render-process-gone", () => closePrintWindow());
  target = created;
  return created;
}

async function layOut(rendererOrigin: string, html: string) {
  const current = acquire(rendererOrigin);
  try {
    await current.loaded;
    if (current.html !== html) {
      current.html = undefined;
      await current.window.webContents.executeJavaScript(writePacket(html));
      current.html = html;
    }
    return current.window;
  } catch (error) {
    closePrintWindow();
    throw error;
  }
}

function scheduleClose() {
  clearTimeout(idleTimer);
  idleTimer = setTimeout(closePrintWindow, IDLE_CLOSE_MS);
  idleTimer.unref();
}

export function closePrintWindow() {
  clearTimeout(idleTimer);
  const current = target;
  target = undefined;
  if (current && !current.window.isDestroyed()) current.window.destroy();
}

export function renderPrintPdf(rendererOrigin: string, html: string) {
  return serialize(async () => {
    try {
      const window = await layOut(rendererOrigin, html);
      const pdf = await window.webContents.printToPDF({
        preferCSSPageSize: true,
        printBackground: true,
      });
      return new Uint8Array(pdf);
    } finally {
      scheduleClose();
    }
  });
}

export function printPacket(rendererOrigin: string, html: string) {
  return serialize(async () => {
    try {
      const window = await layOut(rendererOrigin, html);
      return await new Promise<PrintResult>((resolve) => {
        window.webContents.print({ printBackground: true }, (printed, failureReason) =>
          resolve(printed ? { printed } : { printed, failureReason }),
        );
      });
    } finally {
      scheduleClose();
    }
  });
}

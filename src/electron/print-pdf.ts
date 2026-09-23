import { BrowserWindow } from "electron";

// A blank page served from the renderer's own origin, so the packet's relative font URL resolves
// exactly as it does when the renderer prints through its hidden iframe.
const PRINT_PAGE_PATH = "/print.html";

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

/**
 * Lays out a print packet in an invisible window and returns Chromium's paginated PDF. Page sizes
 * and margins come from the packet's own `@page` rules, so the result matches the printed output.
 */
export async function renderPrintPdf(rendererOrigin: string, html: string) {
  const window = new BrowserWindow({
    show: false,
    width: 900,
    height: 1200,
    webPreferences: { contextIsolation: true, nodeIntegration: false, sandbox: true },
  });

  try {
    await window.loadURL(new URL(PRINT_PAGE_PATH, rendererOrigin).href);
    await window.webContents.executeJavaScript(writePacket(html));
    const pdf = await window.webContents.printToPDF({
      preferCSSPageSize: true,
      printBackground: true,
    });
    return new Uint8Array(pdf);
  } finally {
    window.destroy();
  }
}

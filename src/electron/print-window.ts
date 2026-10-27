import { Context, Effect, Layer, RcRef, Semaphore } from "effect";
import { BrowserWindow } from "electron";
import type { PrintResult } from "#/desktop-window";

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
})()`;

const openPrintPage = (rendererOrigin: string) =>
  Effect.acquireRelease(
    Effect.sync(() => ({
      window: new BrowserWindow({
        show: false,
        width: 900,
        height: 1200,
        webPreferences: { contextIsolation: true, nodeIntegration: false, sandbox: true },
      }),
      html: undefined as string | undefined,
    })),
    ({ window }) =>
      Effect.sync(() => {
        if (!window.isDestroyed()) window.destroy();
      }),
  ).pipe(
    Effect.tap(({ window }) =>
      Effect.promise(() => window.loadURL(new URL("/print.html", rendererOrigin).href)),
    ),
  );

export class PrintWindow extends Context.Service<
  PrintWindow,
  {
    readonly renderPdf: (html: string) => Effect.Effect<Uint8Array>;
    readonly print: (html: string) => Effect.Effect<PrintResult>;
    readonly close: Effect.Effect<void>;
  }
>()("missal/PrintWindow") {
  static readonly layer = (rendererOrigin: string) =>
    Layer.effect(
      PrintWindow,
      Effect.gen(function* () {
        const page = yield* RcRef.make({
          acquire: openPrintPage(rendererOrigin),
          idleTimeToLive: "2 minutes",
        });
        const lock = yield* Semaphore.make(1);

        const withPacket = <A>(html: string, use: (window: BrowserWindow) => Promise<A>) =>
          Effect.gen(function* () {
            const current = yield* RcRef.get(page);
            if (current.html !== html) {
              current.html = undefined;
              yield* Effect.promise(() =>
                current.window.webContents.executeJavaScript(writePacket(html)),
              );
              current.html = html;
            }
            return yield* Effect.promise(() => use(current.window));
          }).pipe(
            Effect.tapCause(() => RcRef.invalidate(page)),
            Effect.scoped,
            (effect) => lock.withPermit(effect),
          );

        return PrintWindow.of({
          renderPdf: (html) =>
            withPacket(html, (window) =>
              window.webContents
                .printToPDF({ preferCSSPageSize: true, printBackground: true })
                .then((pdf) => new Uint8Array(pdf)),
            ),
          print: (html) =>
            withPacket(
              html,
              (window) =>
                new Promise<PrintResult>((resolve) =>
                  window.webContents.print({ printBackground: true }, (printed, failureReason) =>
                    resolve(printed ? { printed } : { printed, failureReason }),
                  ),
                ),
            ),
          close: RcRef.invalidate(page),
        });
      }),
    );
}

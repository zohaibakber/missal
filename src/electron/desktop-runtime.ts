import { Effect, Layer, ManagedRuntime } from "effect";
import type { MessagePortMain } from "electron";
import type { DesktopPreferences } from "#/desktop-window";
import { PreferencesStore } from "#/electron/preferences";
import { PrintWindow } from "#/electron/print-window";
import type { StorageWorkerConfig } from "#/electron/storage-control-rpc";
import { StorageWorker } from "#/electron/storage-worker-client";

export function startDesktopRuntime(options: {
  workerPath: string;
  config: StorageWorkerConfig;
  rendererOrigin: string;
  preferencesDirectory: string;
}) {
  const runtime = ManagedRuntime.make(
    Layer.mergeAll(
      StorageWorker.layer(options),
      PrintWindow.layer(options.rendererOrigin),
      PreferencesStore.layer(options.preferencesDirectory),
    ),
  );

  return {
    preferences: runtime.runPromise(PreferencesStore.use(Effect.succeed)),
    updatePreferences: (f: (preferences: DesktopPreferences) => DesktopPreferences) =>
      runtime.runFork(PreferencesStore.use((store) => store.update(f))),
    ready: runtime.runPromise(StorageWorker.use((storage) => storage.open)),
    connect: (renderer: MessagePortMain) =>
      runtime.runPromise(StorageWorker.use((storage) => storage.connect(renderer))),
    renderPdf: (html: string) =>
      runtime.runPromise(PrintWindow.use((print) => print.renderPdf(html))),
    print: (html: string) => runtime.runPromise(PrintWindow.use((print) => print.print(html))),
    closePrintWindow: () => runtime.runFork(PrintWindow.use((print) => print.close)),
    dispose: () => runtime.dispose(),
  };
}

export type DesktopRuntime = ReturnType<typeof startDesktopRuntime>;

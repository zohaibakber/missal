import { app, BrowserWindow, dialog, ipcMain, nativeTheme, net, protocol } from "electron";
import squirrelStartup from "electron-squirrel-startup";
import { updateElectronApp, UpdateSourceType } from "update-electron-app";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import {
  DESKTOP_PRINT_CHANNEL,
  DESKTOP_PRINT_PDF_CHANNEL,
  DESKTOP_THEME_CHANNEL,
  type DesktopTheme,
} from "./desktop-window";
import { closePrintWindow, printPacket, renderPrintPdf } from "./electron/print-pdf";
import { registerStorageIpc } from "./electron/storage-ipc-main";
import type { StorageHost } from "./electron/storage-worker-client";

const RENDERER_SCHEME = "missal";
const RENDERER_HOST = "renderer";
// Matches --titlebar-height in styles.css.
const TITLEBAR_HEIGHT = 44;
const TITLEBAR_COLOR = "#01000000";
const TITLEBAR_LIGHT_SYMBOL_COLOR = "#404040";
const TITLEBAR_DARK_SYMBOL_COLOR = "#d4d4d4";

// Matches --sidebar in styles.css so the window never flashes a different shade behind the chrome.
const windowBackground = () => (nativeTheme.shouldUseDarkColors ? "#0c0c0c" : "#f5f5f5");

const titleBarOptions = (): Pick<
  Electron.BrowserWindowConstructorOptions,
  "titleBarOverlay" | "titleBarStyle"
> => {
  return {
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: TITLEBAR_COLOR,
      height: TITLEBAR_HEIGHT,
      symbolColor: nativeTheme.shouldUseDarkColors
        ? TITLEBAR_DARK_SYMBOL_COLOR
        : TITLEBAR_LIGHT_SYMBOL_COLOR,
    },
  };
};

protocol.registerSchemesAsPrivileged([
  {
    scheme: RENDERER_SCHEME,
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
      corsEnabled: true,
      stream: true,
      codeCache: true,
    },
  },
]);

// App windows only: the hidden print window has no title bar overlay and must not keep the app open.
const appWindows = new Set<BrowserWindow>();

const updateWindowAppearance = (window: BrowserWindow) => {
  if (window.isDestroyed()) return;

  window.setBackgroundColor(windowBackground());
  window.setTitleBarOverlay({
    color: TITLEBAR_COLOR,
    height: TITLEBAR_HEIGHT,
    symbolColor: nativeTheme.shouldUseDarkColors
      ? TITLEBAR_DARK_SYMBOL_COLOR
      : TITLEBAR_LIGHT_SYMBOL_COLOR,
  });
};

const createWindow = () => {
  const mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 800,
    minHeight: 600,
    show: false,
    autoHideMenuBar: true,
    backgroundColor: windowBackground(),
    title: "Missal",
    ...titleBarOptions(),
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      v8CacheOptions: "bypassHeatCheck",
    },
  });

  appWindows.add(mainWindow);
  mainWindow.once("ready-to-show", () => {
    mainWindow.show();
  });
  mainWindow.once("closed", () => {
    appWindows.delete(mainWindow);
    if (appWindows.size === 0) closePrintWindow();
  });

  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    void mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
    return;
  }

  void mainWindow.loadURL(`${RENDERER_SCHEME}://${RENDERER_HOST}/`);
};

const isFile = (filePath: string) =>
  fs.stat(filePath).then(
    (stats) => stats.isFile(),
    () => false,
  );

const handleRendererProtocol = async (request: Request) => {
  const url = new URL(request.url);
  const rendererRoot = path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}`);
  const requestedPath = url.pathname === "/" ? "index.html" : decodeURIComponent(url.pathname);
  const filePath = path.normalize(path.join(rendererRoot, requestedPath));
  const isInsideRenderer =
    filePath === rendererRoot || filePath.startsWith(`${rendererRoot}${path.sep}`);

  if (!isInsideRenderer) {
    return new Response("Forbidden", { status: 403 });
  }

  const fallback = path.join(rendererRoot, "index.html");
  const resolved = (await isFile(filePath)) ? filePath : fallback;

  return net.fetch(pathToFileURL(resolved).toString());
};

const isDesktopTheme = (theme: unknown): theme is DesktopTheme => {
  return theme === "dark" || theme === "light" || theme === "system";
};

const rendererOrigin = () =>
  MAIN_WINDOW_VITE_DEV_SERVER_URL
    ? new URL(MAIN_WINDOW_VITE_DEV_SERVER_URL).origin
    : `${RENDERER_SCHEME}://${RENDERER_HOST}`;

const registerDesktopIntegration = () => {
  ipcMain.on(DESKTOP_THEME_CHANNEL, (_event, theme: unknown) => {
    if (isDesktopTheme(theme)) nativeTheme.themeSource = theme;
  });

  const printRequest = (event: Electron.IpcMainInvokeEvent, html: unknown) => {
    const origin = rendererOrigin();
    const senderUrl = event.senderFrame?.url;
    if (typeof html !== "string" || !senderUrl || new URL(senderUrl).origin !== origin) {
      throw new Error("Print request rejected");
    }
    return { origin, html };
  };

  ipcMain.handle(DESKTOP_PRINT_PDF_CHANNEL, (event, html: unknown) => {
    const request = printRequest(event, html);
    return renderPrintPdf(request.origin, request.html);
  });

  ipcMain.handle(DESKTOP_PRINT_CHANNEL, (event, html: unknown) => {
    const request = printRequest(event, html);
    return printPacket(request.origin, request.html);
  });
};

const startApp = () => {
  // Auto-update does nothing in development or outside Windows.
  updateElectronApp({
    updateSource: {
      type: UpdateSourceType.ElectronPublicUpdateService,
      repo: "zohaibakber/missal",
    },
  });

  let storage: Promise<StorageHost> | undefined;
  let disposingStorage = false;

  void app.whenReady().then(() => {
    protocol.handle(RENDERER_SCHEME, handleRendererProtocol);
    registerDesktopIntegration();
    createWindow();

    // Effect, RPC and the storage schemas load only after the window exists, so compiling them
    // doesn't delay first paint. The channel is registered now and its requests wait for them.
    storage = import("./electron/storage-worker-client").then((client) =>
      client.startStorageWorker({
        workerPath: path.join(__dirname, "storage-worker.cjs"),
        config: {
          databasePath: client.resolveDatabasePath(app.getPath("userData")),
          migrationsFolder: client.resolveMigrationsFolder({
            packaged: app.isPackaged,
            resourcesPath: process.resourcesPath,
            cwd: process.cwd(),
          }),
        },
      }),
    );
    registerStorageIpc(storage);
    storage
      .then((host) => host.ready)
      .catch((error: unknown) => {
        dialog.showErrorBox(
          "Missal could not open the database",
          error instanceof Error ? error.message : "Storage operation failed",
        );
        app.quit();
      });
  });

  nativeTheme.on("updated", () => {
    for (const window of appWindows) {
      updateWindowAppearance(window);
    }
  });

  app.on("window-all-closed", () => {
    if (process.platform !== "darwin") {
      app.quit();
    }
  });

  app.on("activate", () => {
    if (appWindows.size === 0) {
      createWindow();
    }
  });

  app.on("before-quit", (event) => {
    if (!storage || disposingStorage) {
      return;
    }

    event.preventDefault();
    disposingStorage = true;
    void storage
      .then((host) => host.dispose())
      .finally(() => {
        app.quit();
      });
  });
};

if (squirrelStartup) {
  app.quit();
} else {
  startApp();
}

import { app, BrowserWindow, dialog, ipcMain, nativeTheme, net, protocol } from "electron";
import squirrelStartup from "electron-squirrel-startup";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { DESKTOP_THEME_CHANNEL, type DesktopTheme } from "./desktop-window";
import { initializeDatabase } from "./electron/database";
import {
  makeElectronMainRuntime,
  resolveDatabasePath,
  resolveMigrationsFolder,
} from "./electron/main-runtime";
import { registerStorageIpc } from "./electron/storage-ipc-main";

const RENDERER_SCHEME = "missal";
const RENDERER_HOST = "renderer";
const TITLEBAR_HEIGHT = 40;
const TITLEBAR_COLOR = "#01000000";
const TITLEBAR_LIGHT_SYMBOL_COLOR = "#1f2937";
const TITLEBAR_DARK_SYMBOL_COLOR = "#f8fafc";

const windowBackground = () => (nativeTheme.shouldUseDarkColors ? "#0a0a0a" : "#ffffff");

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
    },
  },
]);

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
    },
  });

  mainWindow.once("ready-to-show", () => {
    mainWindow.show();
  });

  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    void mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
    return;
  }

  void mainWindow.loadURL(`${RENDERER_SCHEME}://${RENDERER_HOST}/`);
};

const handleRendererProtocol = (request: Request) => {
  const url = new URL(request.url);
  const rendererRoot = path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}`);
  const requestedPath = url.pathname === "/" ? "index.html" : decodeURIComponent(url.pathname);
  const filePath = path.normalize(path.join(rendererRoot, requestedPath));
  const isInsideRenderer =
    filePath === rendererRoot || filePath.startsWith(`${rendererRoot}${path.sep}`);

  if (!isInsideRenderer) {
    return Promise.resolve(new Response("Forbidden", { status: 403 }));
  }

  const fallback = path.join(rendererRoot, "index.html");
  const resolved = fs.existsSync(filePath) && fs.statSync(filePath).isFile() ? filePath : fallback;

  return net.fetch(pathToFileURL(resolved).toString());
};

const isDesktopTheme = (theme: unknown): theme is DesktopTheme => {
  return theme === "dark" || theme === "light" || theme === "system";
};

const registerDesktopIntegration = () => {
  ipcMain.on(DESKTOP_THEME_CHANNEL, (_event, theme: unknown) => {
    if (isDesktopTheme(theme)) nativeTheme.themeSource = theme;
  });
};

const startApp = () => {
  let storageRuntime: ReturnType<typeof makeElectronMainRuntime> | undefined;
  let disposingStorage = false;

  void app.whenReady().then(async () => {
    protocol.handle(RENDERER_SCHEME, handleRendererProtocol);
    registerDesktopIntegration();

    storageRuntime = makeElectronMainRuntime({
      databasePath: resolveDatabasePath(app.getPath("userData")),
      migrationsFolder: resolveMigrationsFolder({
        packaged: app.isPackaged,
        resourcesPath: process.resourcesPath,
        cwd: process.cwd(),
      }),
    });

    try {
      await storageRuntime.runPromise(initializeDatabase);
    } catch (error) {
      dialog.showErrorBox(
        "Missal could not open the database",
        error instanceof Error ? error.message : "Storage operation failed",
      );
      app.quit();
      return;
    }

    registerStorageIpc(storageRuntime);
    createWindow();
  });

  nativeTheme.on("updated", () => {
    for (const window of BrowserWindow.getAllWindows()) {
      updateWindowAppearance(window);
    }
  });

  app.on("window-all-closed", () => {
    if (process.platform !== "darwin") {
      app.quit();
    }
  });

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });

  app.on("before-quit", (event) => {
    if (!storageRuntime || disposingStorage) {
      return;
    }

    event.preventDefault();
    disposingStorage = true;
    void storageRuntime.dispose().finally(() => {
      app.quit();
    });
  });
};

// Handle creating/removing shortcuts on Windows when installing/uninstalling.
if (squirrelStartup) {
  app.quit();
} else {
  startApp();
}

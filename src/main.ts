import {
  app,
  BrowserWindow,
  dialog,
  ipcMain,
  MessageChannelMain,
  nativeTheme,
  net,
  protocol,
} from "electron";
import { autoUpdater } from "electron-updater";
import { Schema } from "effect";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import {
  DESKTOP_PREFERENCES_CHANNEL,
  DESKTOP_PRINT_CHANNEL,
  DESKTOP_PRINT_PDF_CHANNEL,
  DESKTOP_THEME_CHANNEL,
} from "./desktop-window";
import { type DesktopRuntime, startDesktopRuntime } from "./electron/desktop-runtime";
import { DesktopTheme, type PreferencesStore } from "./electron/preferences";
import { STORAGE_CONNECT_CHANNEL, STORAGE_PORT_CHANNEL } from "./electron/storage-channel";

const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL;
const RENDERER_SCHEME = "missal";
const RENDERER_ORIGIN = DEV_SERVER_URL ? new URL(DEV_SERVER_URL).origin : "missal://renderer";
const RENDERER_ROOT = path.join(__dirname, "../dist");
const TITLEBAR_HEIGHT = 44;

const windowBackground = () => (nativeTheme.shouldUseDarkColors ? "#0d0e11" : "#f3f4f7");

const titleBarOverlay = () => ({
  color: "#01000000",
  height: TITLEBAR_HEIGHT,
  symbolColor: nativeTheme.shouldUseDarkColors ? "#d4d4d4" : "#404040",
});

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

const appWindows = new Set<BrowserWindow>();
let desktop: DesktopRuntime | undefined;
let preferences: PreferencesStore["Service"] | undefined;
let disposing = false;

const createWindow = () => {
  const window = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 800,
    minHeight: 600,
    show: false,
    autoHideMenuBar: true,
    backgroundColor: windowBackground(),
    title: "Missal",
    titleBarStyle: "hidden",
    titleBarOverlay: titleBarOverlay(),
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      v8CacheOptions: "bypassHeatCheck",
    },
  });

  appWindows.add(window);
  window.once("ready-to-show", () => window.show());
  window.once("closed", () => {
    appWindows.delete(window);
    if (appWindows.size === 0) desktop?.closePrintWindow();
  });
  void window.loadURL(DEV_SERVER_URL ?? `${RENDERER_ORIGIN}/`);
};

const serveRenderer = async (request: Request) => {
  const { pathname } = new URL(request.url);
  const filePath = path.join(RENDERER_ROOT, decodeURIComponent(pathname));
  if (path.relative(RENDERER_ROOT, filePath).startsWith("..")) {
    return new Response("Forbidden", { status: 403 });
  }
  const isFile = await fs.stat(filePath).then(
    (stats) => stats.isFile(),
    () => false,
  );
  return net.fetch(pathToFileURL(isFile ? filePath : path.join(RENDERER_ROOT, "index.html")).href);
};

const fromRenderer = (event: Electron.IpcMainEvent | Electron.IpcMainInvokeEvent) =>
  event.senderFrame?.origin === RENDERER_ORIGIN;

const printHtml =
  <A>(run: (runtime: DesktopRuntime, html: string) => Promise<A>) =>
  (event: Electron.IpcMainInvokeEvent, html: unknown) => {
    if (!desktop || typeof html !== "string" || !fromRenderer(event)) {
      throw new Error("Print request rejected");
    }
    return run(desktop, html);
  };

const isDesktopTheme = Schema.is(DesktopTheme);

if (app.isPackaged) void autoUpdater.checkForUpdatesAndNotify();

void app.whenReady().then(async () => {
  protocol.handle(RENDERER_SCHEME, serveRenderer);
  const userData = app.getPath("userData");
  const resources = app.isPackaged ? process.resourcesPath : process.cwd();
  const runtime = startDesktopRuntime({
    workerPath: path.join(__dirname, "storage-worker.cjs"),
    rendererOrigin: RENDERER_ORIGIN,
    preferencesDirectory: userData,
    config: {
      databasePath: path.join(userData, "missal.sqlite"),
      migrationsFolder: path.join(resources, "drizzle"),
      templatesFolder: path.join(resources, "templates"),
    },
  });
  desktop = runtime;
  runtime.ready.catch((error: unknown) => {
    dialog.showErrorBox(
      "Missal could not open the database",
      error instanceof Error ? error.message : "Storage operation failed",
    );
    app.quit();
  });
  preferences = await runtime.preferences;
  nativeTheme.themeSource = preferences.current().theme;
  createWindow();
});

ipcMain.on(DESKTOP_PREFERENCES_CHANNEL, (event) => {
  event.returnValue = fromRenderer(event) ? preferences?.current() : null;
});

ipcMain.on(DESKTOP_THEME_CHANNEL, (event, theme: unknown) => {
  if (!desktop || !fromRenderer(event) || !isDesktopTheme(theme)) return;
  nativeTheme.themeSource = theme;
  desktop.updatePreferences((current) => ({ ...current, theme }));
});

ipcMain.on(STORAGE_CONNECT_CHANNEL, (event) => {
  if (!desktop || !fromRenderer(event)) return;
  const { port1, port2 } = new MessageChannelMain();
  event.sender.postMessage(STORAGE_PORT_CHANNEL, null, [port2]);
  void desktop.connect(port1);
});

ipcMain.handle(
  DESKTOP_PRINT_PDF_CHANNEL,
  printHtml((runtime, html) => runtime.renderPdf(html)),
);
ipcMain.handle(
  DESKTOP_PRINT_CHANNEL,
  printHtml((runtime, html) => runtime.print(html)),
);

nativeTheme.on("updated", () => {
  for (const window of appWindows) {
    window.setBackgroundColor(windowBackground());
    window.setTitleBarOverlay(titleBarOverlay());
  }
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});

app.on("activate", () => {
  if (appWindows.size === 0) createWindow();
});

app.on("before-quit", (event) => {
  if (!desktop || disposing) return;
  event.preventDefault();
  disposing = true;
  void desktop.dispose().finally(() => app.quit());
});

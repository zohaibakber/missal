import { app, BrowserWindow } from "electron";
import fs from "node:fs/promises";
import path from "node:path";
import { gzipSync } from "node:zlib";
import { createServer } from "vite";

async function bundleTemplates() {
  const directory = path.resolve("templates");
  const fields = JSON.parse(await fs.readFile(path.join(directory, "fields.json"), "utf8"));
  const files = (await fs.readdir(directory))
    .filter((name) => name.endsWith(".docx"))
    .sort((a, b) => a.localeCompare(b, "ur"));

  await app.whenReady();
  const server = await createServer({ mode: "test", logLevel: "warn", server: { port: 0 } });
  await server.listen();
  try {
    const window = new BrowserWindow({
      show: false,
      webPreferences: { backgroundThrottling: false },
    });
    await window.loadURL(
      new URL("/scripts/bundle-templates/index.html", server.resolvedUrls.local[0]).href,
    );
    while (!(await window.webContents.executeJavaScript("typeof convertTemplate === 'function'")))
      await new Promise((resolve) => setTimeout(resolve, 100));

    const templates = [];
    for (const file of files) {
      const bytes = (await fs.readFile(path.join(directory, file))).toString("base64");
      const document = await window.webContents.executeJavaScript(
        `convertTemplate(Uint8Array.from(atob("${bytes}"), (c) => c.charCodeAt(0)), ${JSON.stringify(fields)})`,
      );
      templates.push({ name: path.basename(file, ".docx"), document });
      console.log(`converted ${file}`);
    }
    const bundle = gzipSync(JSON.stringify(templates), { level: 9 });
    await fs.writeFile(path.join(directory, "bundle.json.gz"), bundle);
    console.log(`wrote ${templates.length} templates, ${(bundle.length / 1e6).toFixed(1)} MB`);
  } finally {
    await server.close();
    app.quit();
  }
}

void bundleTemplates();

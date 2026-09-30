// Converts every Word file in templates/ into bundled-templates/, which ships with the app.
// Run with `vp run templates:build`; Electron supplies the Chromium the converter needs.
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { gzipSync } from "node:zlib";
import { app, BrowserWindow } from "electron";
import { build } from "vite";

const root = path.resolve(import.meta.dirname, "../..");
const sourceFolder = path.join(root, "templates");
const packFolder = path.join(root, "bundled-templates");
const stagingFolder = path.join(root, ".bundled-templates-staging");

async function bundleConverter() {
  const output = await build({
    configFile: false,
    logLevel: "warn",
    resolve: { alias: { "#": path.join(root, "src") } },
    define: { "process.env.NODE_ENV": JSON.stringify("production") },
    build: {
      write: false,
      minify: false,
      lib: {
        entry: path.join(import.meta.dirname, "convert.ts"),
        formats: ["iife"],
        name: "MissalTemplatePack",
      },
    },
  });
  const chunks = (Array.isArray(output) ? output : [output]).flatMap((result) =>
    "output" in result ? result.output : [],
  );
  const chunk = chunks.find((item) => item.type === "chunk");
  if (!chunk) throw new Error("The converter did not bundle.");
  return chunk.code;
}

async function main() {
  const sources = (await readdir(sourceFolder)).filter((file) => file.endsWith(".docx")).sort();
  const window = new BrowserWindow({ show: false });
  await window.loadURL("data:text/html;charset=utf-8,<!doctype html><html><body></body></html>");
  await window.webContents.executeJavaScript(await bundleConverter());

  // Converted into a staging folder so a failed build keeps the current pack.
  await rm(stagingFolder, { recursive: true, force: true });
  await mkdir(path.join(stagingFolder, "bodies"), { recursive: true });
  const entries = [];
  let problems = 0;
  for (const file of sources) {
    const name = path.basename(file, ".docx").trim();
    const bytes = await readFile(path.join(sourceFolder, file));
    const converted = await window.webContents.executeJavaScript(
      `convertDocx(${JSON.stringify(bytes.toString("base64"))}, ${JSON.stringify(file)})`,
    );
    const json = Buffer.from(JSON.stringify(converted.document));
    const bodyHash = createHash("sha256").update(json).digest("hex");
    await writeFile(
      path.join(stagingFolder, "bodies", `${bodyHash}.json.gz`),
      gzipSync(json, { level: 6 }),
    );
    entries.push({ name, bodyHash, bytes: json.byteLength });

    console.log(`${name}: ${converted.fieldNames.length} fields`);
    if (converted.newFieldNames.length > 0) {
      console.log(`  new custom fields: ${converted.newFieldNames.join("، ")}`);
    }
    for (const notice of converted.notices) console.log(`  note: ${notice}`);
    if (converted.unconvertedTokens.length > 0) {
      problems += 1;
      console.error(`  not converted: ${converted.unconvertedTokens.join(" ")}`);
    }
  }
  if (problems > 0) {
    await rm(stagingFolder, { recursive: true, force: true });
    return 1;
  }

  const packHash = createHash("sha256").update(JSON.stringify(entries)).digest("hex");
  await writeFile(
    path.join(stagingFolder, "manifest.json"),
    `${JSON.stringify({ packHash, entries }, null, 2)}\n`,
  );
  const seedCode = await runSeed(stagingFolder);
  if (seedCode !== 0) {
    await rm(stagingFolder, { recursive: true, force: true });
    return 1;
  }
  await rm(packFolder, { recursive: true, force: true });
  await rename(stagingFolder, packFolder);
  console.log(`Wrote ${entries.length} templates to bundled-templates/`);
  return 0;
}

function runSeed(staging) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "node",
      [
        "--experimental-strip-types",
        "--import",
        path.join(root, "scripts/performance/register.mjs"),
        path.join(root, "scripts/template-pack/seed.ts"),
        staging,
      ],
      { cwd: root, stdio: "inherit", env: process.env },
    );
    child.on("error", reject);
    child.on("exit", (code) => resolve(code ?? 1));
  });
}

app.disableHardwareAcceleration();
app
  .whenReady()
  .then(main)
  .then(
    (code) => app.exit(code),
    (error) => {
      console.error(error);
      app.exit(1);
    },
  );

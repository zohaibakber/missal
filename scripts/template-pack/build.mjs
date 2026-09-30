// Converts every Word file in templates/ into bundled-templates/, which ships with the app.
// Run with `vp run templates:build`; Electron supplies the Chromium the converter needs.
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { app, BrowserWindow } from "electron";
import { build } from "vite";

const root = path.resolve(import.meta.dirname, "../..");
const sourceFolder = path.join(root, "templates");
const packFolder = path.join(root, "bundled-templates");

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

  await rm(packFolder, { recursive: true, force: true });
  await mkdir(packFolder, { recursive: true });
  const index = [];
  let problems = 0;
  for (const file of sources) {
    const name = path.basename(file, ".docx").trim();
    const bytes = await readFile(path.join(sourceFolder, file));
    const converted = await window.webContents.executeJavaScript(
      `convertDocx(${JSON.stringify(bytes.toString("base64"))}, ${JSON.stringify(file)})`,
    );
    await writeFile(
      path.join(packFolder, `${name}.json`),
      `${JSON.stringify(converted.document)}\n`,
    );
    index.push({ name, sourceHash: createHash("sha256").update(bytes).digest("hex") });

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
  await writeFile(path.join(packFolder, "index.json"), `${JSON.stringify(index, null, 2)}\n`);
  console.log(`Wrote ${index.length} templates to bundled-templates/`);
  return problems === 0 ? 0 : 1;
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

// Converts every Word file in templates/ into bundled-templates/, which ships with the app.
// Run with `vp run templates:build`; Electron supplies the Chromium the converter needs.
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, readdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { gzipSync } from "node:zlib";
import { app, BrowserWindow } from "electron";
import { build } from "vite";
import {
  TEMPLATE_PACK_FORMAT_VERSION,
  TEMPLATE_PACK_GZIP_LEVEL,
  TEMPLATE_PACK_MANIFEST,
  TEMPLATE_PACK_SCHEMA_VERSION,
  templatePackHash,
  validatePack,
} from "./format.mjs";

const root = path.resolve(import.meta.dirname, "../..");
const sourceFolder = path.join(root, "templates");
const packFolder = path.join(root, "bundled-templates");
const stagingFolder = path.join(root, ".bundled-templates-staging");
const previousFolder = path.join(root, ".bundled-templates-previous");

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

async function publishPack() {
  await rm(previousFolder, { recursive: true, force: true });
  const hadPack = existsSync(packFolder);
  if (hadPack) await rename(packFolder, previousFolder);
  try {
    await rename(stagingFolder, packFolder);
  } catch (error) {
    if (hadPack && !existsSync(packFolder) && existsSync(previousFolder)) {
      await rename(previousFolder, packFolder);
    }
    throw error;
  }
  await rm(previousFolder, { recursive: true, force: true });
}

async function main() {
  const sources = (await readdir(sourceFolder)).filter((file) => file.endsWith(".docx")).sort();
  const window = new BrowserWindow({ show: false });
  await window.loadURL("data:text/html;charset=utf-8,<!doctype html><html><body></body></html>");
  await window.webContents.executeJavaScript(await bundleConverter());

  await rm(stagingFolder, { recursive: true, force: true });
  await mkdir(path.join(stagingFolder, "bodies"), { recursive: true });
  const entries = [];
  let problems = 0;
  let converterVersion;
  try {
    for (const file of sources) {
      const name = path.basename(file, ".docx").trim();
      const bytes = await readFile(path.join(sourceFolder, file));
      const converted = await window.webContents.executeJavaScript(
        `convertDocx(${JSON.stringify(bytes.toString("base64"))}, ${JSON.stringify(file)})`,
      );
      converterVersion ??= converted.converterVersion;
      if (converted.converterVersion !== converterVersion) {
        problems += 1;
        console.error(`${name}: converter version ${converted.converterVersion} does not match`);
      }
      const json = Buffer.from(JSON.stringify(converted.document));
      const bodyHash = createHash("sha256").update(json).digest("hex");
      await writeFile(
        path.join(stagingFolder, "bodies", `${bodyHash}.json.gz`),
        gzipSync(json, { level: TEMPLATE_PACK_GZIP_LEVEL }),
      );
      entries.push({
        bodyHash,
        decodedBytes: json.byteLength,
        displayName: name,
        fieldNames: [...new Set(converted.fieldNames)].sort((left, right) =>
          left.localeCompare(right),
        ),
        name,
        sourceHash: createHash("sha256").update(bytes).digest("hex"),
      });

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

    entries.sort((left, right) => left.name.localeCompare(right.name));
    const manifest = {
      converterVersion: converterVersion ?? 1,
      entries,
      formatVersion: TEMPLATE_PACK_FORMAT_VERSION,
      schemaVersion: TEMPLATE_PACK_SCHEMA_VERSION,
    };
    const packHash = templatePackHash(manifest);
    await writeFile(
      path.join(stagingFolder, TEMPLATE_PACK_MANIFEST),
      `${JSON.stringify({ ...manifest, packHash }, null, 2)}\n`,
    );
    const errors = validatePack(stagingFolder);
    if (problems > 0 || errors.length > 0) {
      throw new Error(
        errors.length > 0 ? errors.join("\n") : "Template conversion reported unconverted tokens",
      );
    }
    await publishPack();
    console.log(`Wrote ${entries.length} templates to bundled-templates/`);
    return 0;
  } finally {
    if (existsSync(stagingFolder)) await rm(stagingFolder, { recursive: true, force: true });
  }
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

import { expect, it } from "@effect/vitest";
import { Effect, Schema } from "effect";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { makeStorageRuntime } from "#/electron/storage-runtime";
import {
  BUNDLED_TEMPLATES_INDEX,
  BundledTemplateIndex,
  bundledTemplateFileName,
} from "#/lib/bundled-templates";
import { DocumentEnvelope, emptyDocumentEnvelope, projectDocument } from "#/lib/document-format";
import { PlaceholderUpdateInput } from "#/lib/placeholder";
import { TemplateUpdateInput } from "#/lib/templates";
import { PlaceholderRepository, TemplateRepository } from "#/repositories/index";

function fieldDocument(...names: string[]) {
  const envelope = emptyDocumentEnvelope();
  return {
    ...Schema.encodeSync(DocumentEnvelope)(envelope),
    state: {
      root: {
        type: "root",
        version: 1,
        direction: "rtl",
        format: "",
        indent: 0,
        children: [
          {
            type: "paragraph",
            version: 1,
            direction: "rtl",
            format: "",
            indent: 0,
            textFormat: 0,
            textStyle: "",
            children: names.map((text) => ({
              type: "field",
              version: 1,
              reference: { _tag: "UnresolvedToken", text },
            })),
          },
        ],
      },
    },
  };
}

function writePack(folder: string, templates: { name: string; hash: string; fields: string[] }[]) {
  rmSync(folder, { recursive: true, force: true });
  mkdirSync(folder, { recursive: true });
  for (const template of templates) {
    writeFileSync(
      join(folder, bundledTemplateFileName(template.name)),
      JSON.stringify(fieldDocument(...template.fields)),
    );
  }
  writeFileSync(
    join(folder, BUNDLED_TEMPLATES_INDEX),
    JSON.stringify(templates.map(({ name, hash }) => ({ name, sourceHash: hash }))),
  );
}

async function withPack(
  run: (open: () => ReturnType<typeof makeStorageRuntime>, pack: string) => Promise<void>,
) {
  const directory = mkdtempSync(join(tmpdir(), "missal-bundled-"));
  const pack = join(directory, "pack");
  const open = () =>
    makeStorageRuntime({
      databasePath: join(directory, "test.sqlite"),
      migrationsFolder: resolve("drizzle"),
      bundledTemplatesFolder: pack,
    });
  try {
    await run(open, pack);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

const listTemplates = Effect.flatMap(TemplateRepository, (repo) => repo.list);
const listFields = Effect.flatMap(PlaceholderRepository, (repo) => repo.list);

it("installs bundled templates once, linking names to the install's fields", () =>
  withPack(async (open, pack) => {
    writePack(pack, [{ name: "ریمانڈ", hash: "a", fields: ["تھانہ نام", "نیا خانہ"] }]);
    for (let launch = 0; launch < 2; launch += 1) {
      const runtime = open();
      try {
        const templates = await runtime.runPromise(listTemplates);
        expect(templates.map((template) => template.name)).toEqual(["ریمانڈ"]);
        const fields = await runtime.runPromise(listFields);
        const station = fields.find((field) => field.label === "تھانہ نام");
        const added = fields.filter((field) => field.label === "نیا خانہ");
        expect(added).toHaveLength(1);
        expect(added[0]?.source).toEqual({ _tag: "Custom" });
        const [summary] = templates;
        if (!summary || !station || !added[0]) throw new Error("Missing template or fields");
        const record = await runtime.runPromise(
          Effect.flatMap(TemplateRepository, (repo) => repo.get(summary.id)),
        );
        expect(projectDocument(record.document).fieldReferences).toEqual([
          { _tag: "CatalogField", id: station.id },
          { _tag: "CatalogField", id: added[0].id },
        ]);
      } finally {
        await runtime.dispose();
      }
    }
  }));

it("updates untouched templates, keeps edited ones, and never restores deleted ones", () =>
  withPack(async (open, pack) => {
    writePack(pack, [
      { name: "اول", hash: "1", fields: ["جرم"] },
      { name: "دوم", hash: "1", fields: ["جرم"] },
      { name: "سوم", hash: "1", fields: ["جرم"] },
    ]);
    let runtime = open();
    const byName = async () =>
      new Map(
        (await runtime.runPromise(listTemplates)).map((template) => [template.name, template]),
      );
    try {
      const installed = await byName();
      const edited = installed.get("دوم");
      const deleted = installed.get("سوم");
      if (!edited || !deleted) throw new Error("Missing installed templates");
      await runtime.runPromise(
        Effect.flatMap(TemplateRepository, (repo) =>
          repo.save(
            new TemplateUpdateInput({
              id: edited.id,
              expectedRevision: edited.revision,
              name: edited.name,
              document: emptyDocumentEnvelope(),
            }),
          ),
        ),
      );
      await runtime.runPromise(
        Effect.flatMap(TemplateRepository, (repo) => repo.remove(deleted.id)),
      );
      // A user renaming a default field must not unlink it from bundled templates.
      const officer = (await runtime.runPromise(listFields)).find(
        (field) => field.label === "تفتیشی",
      );
      if (!officer) throw new Error("Missing officer field");
      await runtime.runPromise(
        Effect.flatMap(PlaceholderRepository, (repo) =>
          repo.update(new PlaceholderUpdateInput({ id: officer.id, label: "آئی او" })),
        ),
      );
      await runtime.dispose();

      writePack(pack, [
        { name: "اول", hash: "2", fields: ["جرم", "تفتیشی"] },
        { name: "دوم", hash: "2", fields: ["جرم", "تفتیشی"] },
        { name: "سوم", hash: "2", fields: ["جرم", "تفتیشی"] },
      ]);
      runtime = open();
      const updated = await byName();
      expect([...updated.keys()].sort()).toEqual(["اول", "دوم"]);
      expect(updated.get("اول")?.fieldCount).toBe(2);
      expect(updated.get("اول")?.revision).toBe(2);
      expect(updated.get("دوم")?.fieldCount).toBe(0);
      const first = updated.get("اول");
      if (!first) throw new Error("Missing updated template");
      const record = await runtime.runPromise(
        Effect.flatMap(TemplateRepository, (repo) => repo.get(first.id)),
      );
      expect(projectDocument(record.document).fieldReferences).toContainEqual({
        _tag: "CatalogField",
        id: officer.id,
      });
      expect((await runtime.runPromise(listFields)).some((field) => field.label === "تفتیشی")).toBe(
        false,
      );
    } finally {
      await runtime.dispose();
    }
  }));

it("ships a converted document for every Word template, built from its current file", () => {
  const sources = readdirSync(resolve("templates")).filter((file) => file.endsWith(".docx"));
  const index = Schema.decodeUnknownSync(BundledTemplateIndex)(
    JSON.parse(readFileSync(resolve("bundled-templates", BUNDLED_TEMPLATES_INDEX), "utf8")),
  );
  expect(index.map((entry) => entry.name).sort()).toEqual(
    sources.map((file) => basename(file, ".docx").trim()).sort(),
  );
  for (const entry of index) {
    const source = readFileSync(resolve("templates", `${entry.name}.docx`));
    // Stale? Run `vp run templates:build`.
    expect(createHash("sha256").update(source).digest("hex"), entry.name).toBe(entry.sourceHash);
    const document = Schema.decodeUnknownSync(DocumentEnvelope)(
      JSON.parse(
        readFileSync(resolve("bundled-templates", bundledTemplateFileName(entry.name)), "utf8"),
      ),
    );
    expect(projectDocument(document).fieldCount).toBeGreaterThan(0);
    // Converted with the app's HTML import, so Word's fonts and sizes survive.
    expect(JSON.stringify(document.state)).toMatch(/font-size:/);
  }
});

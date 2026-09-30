import { expect, it } from "@effect/vitest";
import { Effect, Schema } from "effect";
import { sql } from "drizzle-orm";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { gunzipSync, gzipSync } from "node:zlib";
import { templatePackHash } from "#/electron/bundled-templates";
import { MissalDrizzle } from "#/electron/database";
import { makeStorageRuntime, type TemplatePackInstallerOptions } from "#/electron/storage-runtime";
import {
  TEMPLATE_CONVERTER_VERSION,
  TEMPLATE_PACK_BATCH_BYTES,
  TEMPLATE_PACK_FORMAT_VERSION,
  TEMPLATE_PACK_MANIFEST,
  TEMPLATE_PACK_MAX_DOCUMENT_BYTES,
  TEMPLATE_PACK_SCHEMA_VERSION,
  TemplatePackManifest,
  templatePackBodyPath,
} from "#/lib/bundled-templates";
import { DocumentEnvelope, emptyDocumentEnvelope, projectDocument } from "#/lib/document-format";
import { PlaceholderUpdateInput } from "#/lib/placeholder";
import { StorageError } from "#/lib/storage-errors";
import { TemplateUpdateInput } from "#/lib/templates";
import { PlaceholderRepository, TemplateRepository } from "#/repositories/index";

const sha = (value: string) => createHash("sha256").update(value).digest("hex");

function fieldDocument(names: string[], pad = "") {
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
            children: [
              ...names.map((text) => ({
                type: "field",
                version: 1,
                reference: { _tag: "UnresolvedToken", text },
              })),
              ...(pad === ""
                ? []
                : [
                    {
                      type: "text",
                      version: 1,
                      text: pad,
                      format: 0,
                      style: "",
                      detail: 0,
                      mode: "normal",
                    },
                  ]),
            ],
          },
        ],
      },
    },
  };
}

function documentJson(fields: string[], minBytes = 0) {
  const bare = JSON.stringify(fieldDocument(fields));
  if (minBytes === 0 || Buffer.byteLength(bare) >= minBytes) return bare;
  const withOne = JSON.stringify(fieldDocument(fields, "a"));
  const overhead = Buffer.byteLength(withOne) - Buffer.byteLength(bare) - 1;
  let padLength = Math.max(0, minBytes - Buffer.byteLength(bare) - overhead);
  let json = JSON.stringify(fieldDocument(fields, "a".repeat(padLength)));
  for (let attempt = 0; attempt < 4 && Buffer.byteLength(json) !== minBytes; attempt += 1) {
    padLength += minBytes - Buffer.byteLength(json);
    json = JSON.stringify(fieldDocument(fields, "a".repeat(Math.max(0, padLength))));
  }
  return json;
}

function writePack(
  folder: string,
  templates: {
    name: string;
    source: string;
    fields: string[];
    displayName?: string;
    minBytes?: number;
    corrupt?: boolean;
    claimBytes?: number;
  }[],
) {
  rmSync(folder, { recursive: true, force: true });
  mkdirSync(join(folder, "bodies"), { recursive: true });
  const entries = templates.map((template) => {
    const json = Buffer.from(documentJson(template.fields, template.minBytes ?? 0));
    const bodyHash = createHash("sha256").update(json).digest("hex");
    writeFileSync(
      join(folder, templatePackBodyPath(bodyHash)),
      template.corrupt ? Buffer.from("not gzip") : gzipSync(json),
    );
    return {
      bodyHash,
      decodedBytes: template.claimBytes ?? json.byteLength,
      displayName: template.displayName ?? template.name,
      fieldNames: template.fields,
      name: template.name,
      sourceHash: sha(template.source),
    };
  });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  const manifest = {
    converterVersion: TEMPLATE_CONVERTER_VERSION,
    entries,
    formatVersion: TEMPLATE_PACK_FORMAT_VERSION,
    schemaVersion: TEMPLATE_PACK_SCHEMA_VERSION,
  };
  writeFileSync(
    join(folder, TEMPLATE_PACK_MANIFEST),
    JSON.stringify({ ...manifest, packHash: templatePackHash(manifest) }),
  );
  return entries;
}

function countingBodyReads(): TemplatePackInstallerOptions & { readonly reads: () => number } {
  let count = 0;
  return {
    readBody: (folder, bodyHash) =>
      Effect.tryPromise({
        try: async () => {
          count += 1;
          return await readFile(join(folder, templatePackBodyPath(bodyHash)));
        },
        catch: () =>
          new StorageError({
            message: "Could not read template body",
            operation: "bundledTemplates.read",
          }),
      }),
    reads: () => count,
  };
}

async function readFile(file: string) {
  const { readFile: read } = await import("node:fs/promises");
  return read(file);
}

function openPack(directory: string, installer?: TemplatePackInstallerOptions) {
  return makeStorageRuntime(
    {
      databasePath: join(directory, "test.sqlite"),
      migrationsFolder: resolve("drizzle"),
      bundledTemplatesFolder: join(directory, "pack"),
    },
    installer,
  );
}

async function settle(runtime: ReturnType<typeof makeStorageRuntime>) {
  return runtime.runPromise(
    Effect.gen(function* () {
      const repository = yield* TemplateRepository;
      for (let attempt = 0; attempt < 500; attempt += 1) {
        const status = yield* repository.packStatus;
        if (status._tag === "Ready" || status._tag === "PartialFailure") return status;
        yield* Effect.sleep("20 millis");
      }
      return yield* Effect.die("Template pack did not settle");
    }),
  );
}

const listTemplates = Effect.flatMap(TemplateRepository, (repo) => repo.list);
const listFields = Effect.flatMap(PlaceholderRepository, (repo) => repo.list);

async function withPack(
  run: (
    open: (installer?: TemplatePackInstallerOptions) => ReturnType<typeof makeStorageRuntime>,
    pack: string,
    databasePath: string,
  ) => Promise<void>,
) {
  const directory = mkdtempSync(join(tmpdir(), "missal-bundled-"));
  const pack = join(directory, "pack");
  try {
    await run((installer) => openPack(directory, installer), pack, join(directory, "test.sqlite"));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

it("installs bundled templates once, linking names to the install's fields", () =>
  withPack(async (open, pack) => {
    writePack(pack, [{ name: "ریمانڈ", source: "a", fields: ["تھانہ نام", "نیا خانہ"] }]);
    for (let launch = 0; launch < 2; launch += 1) {
      const runtime = open();
      try {
        expect((await settle(runtime))._tag).toBe("Ready");
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

it("does not read bodies or write on an unchanged launch", () =>
  withPack(async (open, pack) => {
    writePack(pack, [{ name: "ریمانڈ", source: "a", fields: ["جرم"] }]);
    const first = open();
    try {
      expect((await settle(first))._tag).toBe("Ready");
    } finally {
      await first.dispose();
    }
    const reader = countingBodyReads();
    const second = open(reader);
    try {
      expect((await settle(second))._tag).toBe("Ready");
      expect(reader.reads()).toBe(0);
      const writes = await second.runPromise(
        Effect.gen(function* () {
          const db = yield* MissalDrizzle;
          const row = yield* db.get<{ total: number }>(sql`select total_changes() as total`);
          return Number(row?.total ?? 0);
        }),
      );
      expect(writes).toBe(0);
    } finally {
      await second.dispose();
    }
  }));

it("updates untouched templates, keeps edited ones, and never restores deleted ones", () =>
  withPack(async (open, pack) => {
    writePack(pack, [
      { name: "اول", source: "1", fields: ["جرم"] },
      { name: "دوم", source: "1", fields: ["جرم"] },
      { name: "سوم", source: "1", fields: ["جرم"] },
    ]);
    let runtime = open();
    const byName = async () =>
      new Map(
        (await runtime.runPromise(listTemplates)).map((template) => [template.name, template]),
      );
    try {
      expect((await settle(runtime))._tag).toBe("Ready");
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
        { name: "اول", source: "2", fields: ["جرم", "تفتیشی"] },
        { name: "دوم", source: "2", fields: ["جرم", "تفتیشی"] },
        { name: "سوم", source: "2", fields: ["جرم", "تفتیشی"] },
      ]);
      runtime = open();
      expect((await settle(runtime))._tag).toBe("Ready");
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

it("keeps a template whose entry was removed from the pack", () =>
  withPack(async (open, pack) => {
    writePack(pack, [
      { name: "ماندہ", source: "1", fields: ["جرم"] },
      { name: "ہٹا", source: "1", fields: ["جرم"] },
    ]);
    let runtime = open();
    try {
      expect((await settle(runtime))._tag).toBe("Ready");
      await runtime.dispose();
      writePack(pack, [{ name: "ماندہ", source: "1", fields: ["جرم"] }]);
      runtime = open();
      expect((await settle(runtime))._tag).toBe("Ready");
      const names = (await runtime.runPromise(listTemplates))
        .map((template) => template.name)
        .sort();
      expect(names).toEqual(["ماندہ", "ہٹا"]);
    } finally {
      await runtime.dispose();
    }
  }));

it("installs in batches of four templates and four mebibytes", () =>
  withPack(async (open, pack) => {
    writePack(
      pack,
      ["1", "2", "3", "4", "5"].map((name) => ({ name, source: "1", fields: ["جرم"] })),
    );
    let release: (() => void) | undefined;
    let entered: () => void = () => undefined;
    const secondBatch = new Promise<void>((resolve) => {
      entered = resolve;
    });
    const reader = countingBodyReads();
    const runtime = open({
      ...reader,
      beforeBatch: (index) => {
        if (index !== 1) return Effect.void;
        return Effect.callback<void>((resume) => {
          release = () => resume(Effect.void);
          entered();
        });
      },
    });
    const settled = settle(runtime);
    try {
      await secondBatch;
      expect(reader.reads()).toBe(4);
      release?.();
      expect((await settled)._tag).toBe("Ready");
      expect(reader.reads()).toBe(5);
      expect(await runtime.runPromise(listTemplates)).toHaveLength(5);
    } finally {
      release?.();
      await runtime.dispose();
    }
  }));

it(
  "starts a new batch before decoded input passes four mebibytes",
  () =>
    withPack(async (open, pack) => {
      const bytes = Math.floor(TEMPLATE_PACK_BATCH_BYTES / 2);
      writePack(pack, [
        { name: "1", source: "1", fields: ["جرم"], minBytes: bytes },
        { name: "2", source: "1", fields: ["جرم"], minBytes: bytes },
        { name: "3", source: "1", fields: ["جرم"], minBytes: bytes },
      ]);
      let release: (() => void) | undefined;
      let entered: () => void = () => undefined;
      const secondBatch = new Promise<void>((resolve) => {
        entered = resolve;
      });
      const reader = countingBodyReads();
      const runtime = open({
        ...reader,
        beforeBatch: (index) => {
          if (index !== 1) return Effect.void;
          return Effect.callback<void>((resume) => {
            release = () => resume(Effect.void);
            entered();
          });
        },
      });
      const settled = settle(runtime);
      try {
        await secondBatch;
        expect(reader.reads()).toBe(2);
        release?.();
        expect((await settled)._tag).toBe("Ready");
      } finally {
        release?.();
        await runtime.dispose();
      }
    }),
  20_000,
);

it("reports a partial failure for a corrupt body and still installs the others", () =>
  withPack(async (open, pack, databasePath) => {
    writePack(pack, [
      { name: "درست", source: "1", fields: ["جرم"] },
      { name: "خراب", source: "1", fields: ["تفتیشی"], corrupt: true },
    ]);
    const runtime = open();
    try {
      const status = await settle(runtime);
      expect(status._tag).toBe("PartialFailure");
      expect((await runtime.runPromise(listTemplates)).map((template) => template.name)).toEqual([
        "درست",
      ]);
      await runtime.dispose();
      const database = new DatabaseSync(databasePath);
      const row = database.prepare("select count(*) as n from template_pack_state").get() as {
        n: number;
      };
      database.close();
      expect(row.n).toBe(0);
    } finally {
      await runtime.dispose();
    }
  }));

it("does not read a document larger than the batch budget", () =>
  withPack(async (open, pack) => {
    const entries = writePack(pack, [
      { name: "چھوٹا", source: "1", fields: ["جرم"] },
      {
        name: "بڑا",
        source: "1",
        fields: ["تفتیشی"],
        claimBytes: TEMPLATE_PACK_MAX_DOCUMENT_BYTES + 1,
      },
    ]);
    const oversized = entries.find((entry) => entry.name === "بڑا");
    const reader = countingBodyReads();
    const seen: string[] = [];
    const runtime = open({
      readBody: (folder, bodyHash) => {
        seen.push(bodyHash);
        return reader.readBody?.(folder, bodyHash) ?? Effect.die("missing reader");
      },
    });
    try {
      expect((await settle(runtime))._tag).toBe("PartialFailure");
      expect(seen).not.toContain(oversized?.bodyHash);
      expect((await runtime.runPromise(listTemplates)).map((template) => template.name)).toEqual([
        "چھوٹا",
      ]);
    } finally {
      await runtime.dispose();
    }
  }));

it("does not record an interrupted install as complete", () =>
  withPack(async (open, pack, databasePath) => {
    writePack(pack, [{ name: "ریمانڈ", source: "a", fields: ["جرم"] }]);
    let entered: () => void = () => undefined;
    const started = new Promise<void>((resolve) => {
      entered = resolve;
    });
    const runtime = open({
      beforeBatch: () =>
        Effect.callback<void>((_resume, signal) => {
          entered();
          signal.addEventListener("abort", () => undefined);
        }),
    });
    const pending = settle(runtime);
    await started;
    await runtime.dispose();
    await pending.catch(() => undefined);
    const database = new DatabaseSync(databasePath);
    const packRows = database.prepare("select count(*) as n from template_pack_state").get() as {
      n: number;
    };
    const templates = database.prepare("select count(*) as n from templates").get() as {
      n: number;
    };
    database.close();
    expect(packRows.n).toBe(0);
    expect(templates.n).toBe(0);

    const resumed = open();
    try {
      expect((await settle(resumed))._tag).toBe("Ready");
      expect((await resumed.runPromise(listTemplates)).map((template) => template.name)).toEqual([
        "ریمانڈ",
      ]);
    } finally {
      await resumed.dispose();
    }
  }));

it("does not overwrite a template saved between batches", () =>
  withPack(async (open, pack) => {
    const names = ["1", "2", "3", "4", "5"];
    writePack(
      pack,
      names.map((name) => ({ name, source: "1", fields: ["جرم"] })),
    );
    let runtime = open();
    try {
      expect((await settle(runtime))._tag).toBe("Ready");
      const installed = await runtime.runPromise(listTemplates);
      const target = installed.find((template) => template.name === "5");
      if (!target) throw new Error("Missing template 5");
      await runtime.dispose();
      writePack(
        pack,
        names.map((name) => ({ name, source: "2", fields: ["جرم", "تفتیشی"] })),
      );
      let release: (() => void) | undefined;
      let entered: () => void = () => undefined;
      const secondBatch = new Promise<void>((resolve) => {
        entered = resolve;
      });
      runtime = open({
        beforeBatch: (index) => {
          if (index !== 1) return Effect.void;
          return Effect.callback<void>((resume) => {
            release = () => resume(Effect.void);
            entered();
          });
        },
      });
      const settled = settle(runtime);
      await secondBatch;
      await runtime.runPromise(
        Effect.flatMap(TemplateRepository, (repo) =>
          repo.save(
            new TemplateUpdateInput({
              document: emptyDocumentEnvelope(),
              expectedRevision: target.revision,
              id: target.id,
              name: target.name,
            }),
          ),
        ),
      );
      release?.();
      expect((await settled)._tag).toBe("Ready");
      const saved = (await runtime.runPromise(listTemplates)).find(
        (template) => template.name === "5",
      );
      const updated = (await runtime.runPromise(listTemplates)).find(
        (template) => template.name === "1",
      );
      if (!saved || !updated) throw new Error("Missing templates");
      expect(saved.revision).toBe(2);
      expect(saved.fieldCount).toBe(0);
      expect(updated.revision).toBe(2);
      expect(updated.fieldCount).toBe(2);
    } finally {
      await runtime.dispose();
    }
  }));

it("reconciles rows that only stored a Word hash without restoring edits or deletions", () =>
  withPack(async (open, pack, databasePath) => {
    writePack(pack, [
      { name: "اول", source: "1", fields: ["جرم"] },
      { name: "دوم", source: "1", fields: ["جرم"] },
      { name: "سوم", source: "1", fields: ["جرم"] },
    ]);
    let runtime = open();
    try {
      expect((await settle(runtime))._tag).toBe("Ready");
      const installed = new Map(
        (await runtime.runPromise(listTemplates)).map((template) => [template.name, template]),
      );
      const edited = installed.get("دوم");
      const deleted = installed.get("سوم");
      if (!edited || !deleted) throw new Error("Missing installed templates");
      await runtime.runPromise(
        Effect.flatMap(TemplateRepository, (repo) =>
          repo.save(
            new TemplateUpdateInput({
              document: emptyDocumentEnvelope(),
              expectedRevision: edited.revision,
              id: edited.id,
              name: edited.name,
            }),
          ),
        ),
      );
      await runtime.runPromise(
        Effect.flatMap(TemplateRepository, (repo) => repo.remove(deleted.id)),
      );
      await runtime.dispose();

      const database = new DatabaseSync(databasePath);
      database.exec("delete from template_pack_state");
      database.exec("update bundled_templates set body_hash = null, converter_version = null");
      database.close();

      runtime = open();
      expect((await settle(runtime))._tag).toBe("Ready");
      const names = new Map(
        (await runtime.runPromise(listTemplates)).map((template) => [template.name, template]),
      );
      expect([...names.keys()].sort()).toEqual(["اول", "دوم"]);
      expect(names.get("دوم")?.fieldCount).toBe(0);
      expect(names.get("اول")?.fieldCount).toBe(1);
    } finally {
      await runtime.dispose();
    }
  }));

it("keeps Word-only bundled rows when the pack columns are added", () => {
  const file = join(mkdtempSync(join(tmpdir(), "missal-pack-migration-")), "old.sqlite");
  const database = new DatabaseSync(file);
  try {
    database.exec(`
      CREATE TABLE bundled_templates (
        name text PRIMARY KEY,
        source_hash text NOT NULL,
        template_id integer,
        template_revision integer NOT NULL
      );
      INSERT INTO bundled_templates (name, source_hash, template_id, template_revision)
      VALUES ('ریمانڈ', 'word-only', 4, 2), ('حذف', 'word-only', NULL, 1);
    `);
    const migration = readFileSync(
      resolve("drizzle/20260930030000_template-pack-state/migration.sql"),
      "utf8",
    );
    for (const statement of migration.split("--> statement-breakpoint")) {
      const sqlText = statement.trim();
      if (sqlText !== "") database.exec(sqlText);
    }
    const rows = database
      .prepare(
        "select name, source_hash, template_id, template_revision, body_hash, converter_version from bundled_templates",
      )
      .all() as {
      name: string;
      source_hash: string;
      template_id: number | null;
      template_revision: number;
      body_hash: string | null;
      converter_version: number | null;
    }[];
    const kept = new Map(rows.map((row) => [row.name, row]));
    expect(kept.get("ریمانڈ")).toMatchObject({
      body_hash: null,
      converter_version: null,
      source_hash: "word-only",
      template_id: 4,
      template_revision: 2,
    });
    expect(kept.get("حذف")).toMatchObject({
      body_hash: null,
      converter_version: null,
      template_id: null,
      template_revision: 1,
    });
    expect(database.prepare("select count(*) as n from template_pack_state").get()).toMatchObject({
      n: 0,
    });
  } finally {
    database.close();
    rmSync(dirname(file), { recursive: true, force: true });
  }
});

it("hashes packs the same way as the template build", () => {
  const manifest = {
    converterVersion: TEMPLATE_CONVERTER_VERSION,
    entries: [
      {
        bodyHash: sha("body"),
        decodedBytes: 12,
        displayName: "نام",
        fieldNames: ["جرم"],
        name: "نام",
        sourceHash: sha("word"),
      },
    ],
    formatVersion: TEMPLATE_PACK_FORMAT_VERSION,
    schemaVersion: TEMPLATE_PACK_SCHEMA_VERSION,
  };
  const fromBuild = execFileSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      "import { templatePackHash } from './scripts/template-pack/format.mjs'; process.stdout.write(templatePackHash(JSON.parse(process.env.PACK_HASH_INPUT ?? '{}')))",
    ],
    { encoding: "utf8", env: { ...process.env, PACK_HASH_INPUT: JSON.stringify(manifest) } },
  );
  expect(fromBuild).toBe(templatePackHash(manifest));
});

it("ships a converted document for every Word template, built from its current file", () => {
  const sources = readdirSync(resolve("templates")).filter((file) => file.endsWith(".docx"));
  const packRoot = resolve("bundled-templates");
  expect(readdirSync(packRoot).sort()).toEqual(["bodies", "manifest.json"]);
  for (const file of readdirSync(join(packRoot, "bodies"))) {
    expect(file.endsWith(".json.gz")).toBe(true);
  }
  const manifest = Schema.decodeUnknownSync(TemplatePackManifest)(
    JSON.parse(readFileSync(join(packRoot, TEMPLATE_PACK_MANIFEST), "utf8")),
  );
  expect(templatePackHash(manifest)).toBe(manifest.packHash);
  expect(manifest.entries.map((entry) => entry.name).sort()).toEqual(
    sources.map((file) => basename(file, ".docx").trim()).sort(),
  );
  for (const entry of manifest.entries) {
    const source = readFileSync(resolve("templates", `${entry.name}.docx`));
    expect(createHash("sha256").update(source).digest("hex"), entry.name).toBe(entry.sourceHash);
    const json = gunzipSync(readFileSync(join(packRoot, templatePackBodyPath(entry.bodyHash))));
    expect(json.byteLength).toBe(entry.decodedBytes);
    expect(createHash("sha256").update(json).digest("hex")).toBe(entry.bodyHash);
    const document = Schema.decodeUnknownSync(DocumentEnvelope)(JSON.parse(json.toString("utf8")));
    expect(projectDocument(document).fieldCount).toBeGreaterThan(0);
    expect(JSON.stringify(document.state)).toMatch(/font-size:/);
    expect(entry.displayName).toBe(entry.name);
  }
});

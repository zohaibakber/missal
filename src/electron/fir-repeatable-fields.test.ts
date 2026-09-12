import { expect, it } from "@effect/vitest";
import { Effect, Schema } from "effect";
import { DatabaseSync } from "node:sqlite";
import { readFileSync, readdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { makeElectronMainRuntime } from "#/electron/main-runtime";
import { FirRepository } from "#/repositories/fir-repository";
import { FirCreateInput, FirUpdateInput, FirRecord, createEmptyFirRecord } from "#/lib/fir";
import { resolveFieldValue } from "#/lib/field";
import { createDefaultPlaceholders } from "#/lib/placeholder";

it("migrates legacy text without splitting names, punctuation or line breaks", () => {
  const db = new DatabaseSync(":memory:");
  try {
    const directories = readdirSync(resolve("drizzle")).sort();
    for (const directory of directories.filter((name) => !name.endsWith("fir-repeatable-fields"))) {
      db.exec(readFileSync(resolve("drizzle", directory, "migration.sql"), "utf8"));
    }
    const before = db.prepare("SELECT id, accused, witness FROM fir_records ORDER BY id").all();
    const migration = directories.find((name) => name.endsWith("fir-repeatable-fields"));
    if (!migration) throw new Error("Missing repeatable fields migration");
    db.exec(readFileSync(resolve("drizzle", migration, "migration.sql"), "utf8"));
    const after = db
      .prepare("SELECT id, accused, witness, zimni FROM fir_records ORDER BY id")
      .all();
    expect(after.length).toBe(before.length);
    for (const [index, row] of after.entries()) {
      expect(JSON.parse(String(row.accused))).toEqual([before[index].accused]);
      expect(JSON.parse(String(row.witness))).toEqual(
        String(before[index].witness).trim() ? [before[index].witness] : [],
      );
      expect(JSON.parse(String(row.zimni))).toEqual([]);
    }
    expect(db.prepare("PRAGMA foreign_key_check").all()).toEqual([]);
    expect(db.prepare("SELECT source FROM placeholders WHERE key = 'zimni'").get()).toEqual({
      source: '{"_tag":"FirProperty","property":"zimni"}',
    });
  } finally {
    db.close();
  }
});

it("persists, reopens and renders multiple FIR entries in their original order", async () => {
  const directory = mkdtempSync(join(tmpdir(), "missal-arrays-"));
  const options = {
    databasePath: join(directory, "test.sqlite"),
    migrationsFolder: resolve("drizzle"),
  };
  let runtime = makeElectronMainRuntime(options);
  try {
    const input = Schema.decodeUnknownSync(FirCreateInput)({
      ...createEmptyFirRecord(),
      fir_no: "TEST/26",
      date: "12-09-2026",
      incident_date: "11-09-2026",
      offence: "test",
      accused: ["ملزم اول\nسکونت", "ملزم دوم / عرف"],
      witness: ["گواہ اول", "گواہ دوم"],
      zimni: ["ضمنی اول", "ضمنی دوم"],
    });
    const created = await runtime.runPromise(
      Effect.flatMap(FirRepository, (repo) => repo.create(input)),
    );
    const updated = await runtime.runPromise(
      Effect.flatMap(FirRepository, (repo) =>
        repo.update(
          Schema.decodeUnknownSync(FirUpdateInput)({
            ...Schema.encodeSync(FirRecord)(created),
            witness: ["گواہ دوم"],
            zimni: [...created.zimni, "ضمنی سوم"],
          }),
        ),
      ),
    );
    await runtime.dispose();
    runtime = makeElectronMainRuntime(options);
    const reopened = await runtime.runPromise(
      Effect.flatMap(FirRepository, (repo) => repo.get(created.id)),
    );
    expect(reopened).toEqual(updated);
    for (const key of ["accused", "witness", "zimni"] as const) {
      const field = createDefaultPlaceholders().find((item) => item.key === key);
      if (!field) throw new Error(`Missing ${key} field`);
      expect(resolveFieldValue(field, reopened, [], {})).toEqual({
        _tag: "Resolved",
        text: reopened[key].join("\n"),
      });
    }
  } finally {
    await runtime.dispose();
    rmSync(directory, { recursive: true, force: true });
  }
});

it("rejects empty accused lists and blank repeatable entries at the storage boundary", () => {
  const input = {
    ...createEmptyFirRecord(),
    fir_no: "TEST",
    date: "12-09-2026",
    incident_date: "11-09-2026",
    offence: "test",
    accused: ["name"],
  };
  expect(Schema.is(FirCreateInput)({ ...input, accused: [] })).toBe(false);
  expect(Schema.is(FirCreateInput)({ ...input, witness: [""] })).toBe(false);
  expect(Schema.is(FirCreateInput)({ ...input, zimni: [" "] })).toBe(false);
});

import { expect, it } from "@effect/vitest";
import { DatabaseSync } from "node:sqlite";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

it("renames default fields to the Word template names without changing IDs or custom names", () => {
  const db = new DatabaseSync(":memory:");
  try {
    const directories = readdirSync(resolve("drizzle")).sort();
    const migration = directories.find((name) => name.endsWith("word-template-field-names"));
    if (!migration) throw new Error("Missing Word template field names migration");
    for (const directory of directories.filter((name) => name < migration)) {
      db.exec(readFileSync(resolve("drizzle", directory, "migration.sql"), "utf8"));
    }
    db.exec(`
      UPDATE placeholders SET label = 'میرا تفتیشی' WHERE label = 'تفتیشی افسر';
      INSERT INTO placeholders (label) VALUES ('مدعی مقدمہ');
      UPDATE app_settings SET field_markers = '{"open":"{{","close":"}}"}';
    `);

    db.exec(readFileSync(resolve("drizzle", migration, "migration.sql"), "utf8"));

    const label = (id: number) =>
      db.prepare("SELECT label FROM placeholders WHERE id = ?").get(id)?.label;
    expect([1, 2, 3, 10].map(label)).toEqual([
      "مقدمہ نمبر",
      "Date FIR",
      "تاریخ ووقت وقوعہ",
      "میرا تفتیشی",
    ]);
    expect(
      db.prepare("SELECT source FROM placeholders WHERE label = 'گواہان 2'").get()?.source,
    ).toBe('{"_tag":"FirProperty","property":"witness","index":2}');
    expect(
      db.prepare("SELECT count(*) AS count FROM placeholders WHERE label = 'مدعی مقدمہ'").get(),
    ).toEqual({ count: 1 });
    expect(db.prepare("SELECT field_markers FROM app_settings").get()).toEqual({
      field_markers: '{"open":"{{","close":"}}"}',
    });
  } finally {
    db.close();
  }
});

it("switches the untouched default markers to the Word template's guillemets", () => {
  const db = new DatabaseSync(":memory:");
  try {
    for (const directory of readdirSync(resolve("drizzle")).sort()) {
      db.exec(readFileSync(resolve("drizzle", directory, "migration.sql"), "utf8"));
    }
    expect(db.prepare("SELECT field_markers FROM app_settings").get()).toEqual({
      field_markers: '{"open":"«","close":"»"}',
    });
  } finally {
    db.close();
  }
});

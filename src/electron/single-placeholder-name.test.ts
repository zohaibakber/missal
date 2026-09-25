import { expect, it } from "@effect/vitest";
import { DatabaseSync } from "node:sqlite";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

it("drops placeholder keys without losing FIR values and makes names unique", () => {
  const db = new DatabaseSync(":memory:");
  try {
    const directories = readdirSync(resolve("drizzle")).sort();
    const migration = directories.find((name) => name.endsWith("single-placeholder-name"));
    if (!migration) throw new Error("Missing single placeholder name migration");
    for (const directory of directories.filter((name) => name < migration)) {
      db.exec(readFileSync(resolve("drizzle", directory, "migration.sql"), "utf8"));
    }
    db.exec(`
      INSERT INTO placeholders (id, key, label) VALUES (100, 'custom_a', ' پتہ '), (101, 'custom_b', 'پتہ');
      INSERT INTO fir_records (id, fir_no, date, offence, accused, incident_date, status)
        VALUES (1, '1', '12-09-2026', 'test', '["test"]', '12-09-2026', 'Open');
      INSERT INTO fir_placeholder_values (fir_id, placeholder_id, value, updated_at)
        VALUES (1, 100, 'لاہور', 'now'), (1, 101, 'کراچی', 'now');
    `);

    db.exec(readFileSync(resolve("drizzle", migration, "migration.sql"), "utf8"));

    const columns = db.prepare("PRAGMA table_info(placeholders)").all();
    expect(columns.map((column) => column.name)).toEqual(["id", "label", "source"]);
    expect(
      db.prepare("SELECT id, label FROM placeholders WHERE id IN (100, 101) ORDER BY id").all(),
    ).toEqual([
      { id: 100, label: "پتہ" },
      { id: 101, label: "پتہ 101" },
    ]);
    expect(
      db.prepare("SELECT placeholder_id, value FROM fir_placeholder_values ORDER BY 1").all(),
    ).toEqual([
      { placeholder_id: 100, value: "لاہور" },
      { placeholder_id: 101, value: "کراچی" },
    ]);
    expect(db.prepare("PRAGMA foreign_key_check").all()).toEqual([]);
    expect(() => db.exec("INSERT INTO placeholders (label) VALUES ('پتہ')")).toThrow(/UNIQUE/);
    expect(db.prepare("SELECT field_markers FROM app_settings").get()).toEqual({
      field_markers: '{"open":"@","close":"@"}',
    });
  } finally {
    db.close();
  }
});

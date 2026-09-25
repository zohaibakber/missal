import { expect, it } from "@effect/vitest";
import { Effect, Schema } from "effect";
import { mkdtempSync, rmSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { makeStorageRuntime } from "#/electron/storage-runtime";
import { dispatchStorageRequest } from "#/electron/storage-dispatch";
import { GlobalPlaceholder, SaveGlobalPlaceholdersInput } from "#/lib/global-placeholder";
import { PlaceholderRepository, FirRepository } from "#/repositories/index";
import { FirCreateInput, createEmptyFirRecord } from "#/lib/fir";
import { CatalogFieldReference, fieldDisplayText } from "#/lib/field";

it("saves arbitrary global placeholders atomically and resolves the same values across FIRs after reopening", async () => {
  const directory = mkdtempSync(join(tmpdir(), "missal-globals-"));
  const options = {
    databasePath: join(directory, "test.sqlite"),
    migrationsFolder: resolve("drizzle"),
  };
  let runtime = makeStorageRuntime(options);
  try {
    const initial = await runtime.runPromise(
      Effect.flatMap(PlaceholderRepository, (repo) => repo.listGlobals),
    );
    expect(initial.map((field) => field.label)).toEqual(
      expect.arrayContaining(["تفتیشی افسر", "تھانہ نام", "ضلع نام", "SHO نام", "DSP نام"]),
    );
    const officer = initial.find((field) => field.label === "تفتیشی افسر");
    if (!officer) throw new Error("Missing global officer");
    const saved = await runtime.runPromise(
      Effect.flatMap(PlaceholderRepository, (repo) =>
        repo.saveGlobals(
          Schema.decodeUnknownSync(SaveGlobalPlaceholdersInput)({
            entries: [
              { _tag: "Existing", id: officer.id, label: "تفتیشی افسر", value: "علی" },
              { _tag: "New", label: "دفتر کا پتہ", value: "لاہور\nمرکزی دفتر" },
            ],
          }),
        ),
      ),
    );
    const address = saved.find((field) => field.label === "دفتر کا پتہ");
    if (!address) throw new Error("Missing custom global");
    for (const number of ["GLOBAL-A", "GLOBAL-B"]) {
      const fir = await runtime.runPromise(
        Effect.flatMap(FirRepository, (repo) =>
          repo.create(
            Schema.decodeUnknownSync(FirCreateInput)({
              ...createEmptyFirRecord(),
              fir_no: number,
              date: "12-09-2026",
              incident_date: "12-09-2026",
              offence: "test",
              accused: ["test"],
              investigation_officer: "Legacy case officer",
            }),
          ),
        ),
      );
      const context = await runtime.runPromise(
        Effect.flatMap(FirRepository, (repo) => repo.getValueContext(fir.id)),
      );
      expect(
        fieldDisplayText(
          CatalogFieldReference.make({ id: address.id }),
          context.toPresentation("values"),
        ),
      ).toEqual({ text: "لاہور\nمرکزی دفتر", unresolved: false });
      expect(
        fieldDisplayText(
          CatalogFieldReference.make({ id: officer.id }),
          context.toPresentation("values"),
        ),
      ).toEqual({ text: "علی", unresolved: false });
    }
    const failed = await runtime.runPromiseExit(
      Effect.flatMap(PlaceholderRepository, (repo) =>
        repo.saveGlobals(
          Schema.decodeUnknownSync(SaveGlobalPlaceholdersInput)({
            entries: [
              { _tag: "Existing", id: address.id, label: "Must roll back", value: "changed" },
              { _tag: "New", label: "Must not be inserted", value: "" },
              { _tag: "Existing", id: 999999, label: "Missing", value: "" },
            ],
          }),
        ),
      ),
    );
    expect(failed._tag).toBe("Failure");
    await runtime.dispose();
    runtime = makeStorageRuntime(options);
    const reopened = await runtime.runPromise(
      Effect.flatMap(PlaceholderRepository, (repo) => repo.listGlobals),
    );
    expect(reopened).toEqual(saved);
    const response = await runtime.runPromise(
      dispatchStorageRequest({
        _tag: "Placeholder.saveGlobals",
        input: {
          entries: [{ _tag: "Existing", id: address.id, label: "دفتر", value: "اسلام آباد" }],
        },
      }),
    );
    if (response._tag !== "Success") throw new Error("IPC save failed");
    const ipcRows = Schema.decodeUnknownSync(Schema.Array(GlobalPlaceholder))(
      structuredClone(response.value),
    );
    expect(ipcRows.find((field) => field.id === address.id)).toMatchObject({
      label: "دفتر",
      value: "اسلام آباد",
    });
  } finally {
    await runtime.dispose();
    rmSync(directory, { recursive: true, force: true });
  }
});

it("migrates legacy global values without changing placeholder identities or choosing between conflicting officers", () => {
  const db = new DatabaseSync(":memory:");
  try {
    const folders = readdirSync(resolve("drizzle")).sort();
    const globalMigration = folders.find((name) => name.endsWith("global-placeholders"));
    if (!globalMigration) throw new Error("Missing global migration");
    for (const folder of folders.filter((name) => name < globalMigration))
      db.exec(readFileSync(resolve("drizzle", folder, "migration.sql"), "utf8"));
    db.prepare("UPDATE app_settings SET shared_placeholders = ?").run(
      JSON.stringify({
        policeStation: "Old station",
        shoName: "Old SHO",
        dspName: "Old DSP",
        district: "Lahore",
        police_station: "Current station",
      }),
    );
    db.exec("UPDATE fir_records SET investigation_officer = ''");
    const insertLegacyFir = db.prepare(
      "INSERT INTO fir_records (fir_no,date,offence,accused,incident_date,status,investigation_officer) VALUES (?, '12-09-2026', 'test', '[\"test\"]', '12-09-2026', 'Open', ?)",
    );
    insertLegacyFir.run("MIGRATION-A", "Officer A");
    insertLegacyFir.run("MIGRATION-B", "Officer B");
    const officerBefore = db
      .prepare("SELECT id FROM placeholders WHERE key = 'investigation_officer'")
      .get();
    db.exec(readFileSync(resolve("drizzle", globalMigration, "migration.sql"), "utf8"));
    const officerAfter = db
      .prepare("SELECT id, source FROM placeholders WHERE key = 'investigation_officer'")
      .get();
    expect(officerAfter?.id).toBe(officerBefore?.id);
    expect(JSON.parse(String(officerAfter?.source))).toEqual({
      _tag: "SharedSetting",
      setting: "investigation_officer",
    });
    const values = JSON.parse(
      String(db.prepare("SELECT shared_placeholders FROM app_settings").get()?.shared_placeholders),
    );
    expect(values.investigation_officer).toBeUndefined();
    expect(values).toMatchObject({
      police_station: "Current station",
      sho_name: "Old SHO",
      dsp_name: "Old DSP",
      district: "Lahore",
    });
  } finally {
    db.close();
  }
});

import { expect, it } from "@effect/vitest";
import { Effect, Exit, Schema } from "effect";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { makeStorageRuntime } from "#/electron/storage-runtime";
import { emptyDocumentEnvelope } from "#/lib/document-format";
import { FirCreateInput, FirId, createEmptyFirRecord } from "#/lib/fir";
import { AddFirTemplatesInput } from "#/lib/fir-document";
import { FirDocumentId, PlaceholderId } from "#/lib/ids";
import { FirPlaceholderValueUpsertInput, TemplateCreateInput } from "#/lib/templates";
import {
  FirDocumentRepository,
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  TemplateRepository,
} from "#/repositories/index";

async function withStorage(run: (runtime: ReturnType<typeof makeStorageRuntime>) => Promise<void>) {
  const directory = mkdtempSync(join(tmpdir(), "missal-queries-"));
  const runtime = makeStorageRuntime({
    databasePath: join(directory, "test.sqlite"),
    migrationsFolder: resolve("drizzle"),
  });
  try {
    await run(runtime);
  } finally {
    await runtime.dispose();
    rmSync(directory, { recursive: true, force: true });
  }
}

const firInput = (fir_no: string) =>
  Schema.decodeUnknownSync(FirCreateInput)({
    ...createEmptyFirRecord(),
    fir_no,
    date: "12-09-2026",
    incident_date: "11-09-2026",
    offence: "test",
    accused: ["ملزم"],
  });

it("loads several documents in the order asked for, and fails on a missing one", () =>
  withStorage(async (runtime) => {
    const fir = await runtime.runPromise(
      Effect.flatMap(FirRepository, (repo) => repo.create(firInput("3/26"))),
    );
    const templates = await runtime.runPromise(
      Effect.forEach(["الف", "ب"], (name) =>
        Effect.flatMap(TemplateRepository, (repo) =>
          repo.create(new TemplateCreateInput({ document: emptyDocumentEnvelope(), name })),
        ),
      ),
    );
    const documents = await runtime.runPromise(
      Effect.flatMap(FirDocumentRepository, (repo) =>
        repo.addTemplates(
          new AddFirTemplatesInput({
            firId: fir.id,
            templateIds: templates.map((template) => template.id),
          }),
        ),
      ),
    );
    const ids = documents.map((document) => document.id).toReversed();

    const loaded = await runtime.runPromise(
      Effect.flatMap(FirDocumentRepository, (repo) => repo.getMany(ids)),
    );
    expect(loaded.map((document) => document.id)).toEqual(ids);
    expect(loaded.map((document) => document.title)).toEqual(["ب", "الف"]);

    const missing = await runtime.runPromiseExit(
      Effect.flatMap(FirDocumentRepository, (repo) =>
        repo.getMany([...ids, FirDocumentId.make(9999)]),
      ),
    );
    expect(Exit.isFailure(missing) && JSON.stringify(missing.cause)).toContain("EntityNotFound");
  }));

it("upserts a FIR value in one statement and names the missing owner", () =>
  withStorage(async (runtime) => {
    const fir = await runtime.runPromise(
      Effect.flatMap(FirRepository, (repo) => repo.create(firInput("4/26"))),
    );
    const [placeholder] = await runtime.runPromise(
      Effect.flatMap(PlaceholderRepository, (repo) => repo.list),
    );
    if (!placeholder) throw new Error("Missing seeded placeholder");
    const upsert = (firId: FirId, placeholderId: PlaceholderId, value: string) =>
      runtime.runPromiseExit(
        Effect.flatMap(FirPlaceholderValueRepository, (repo) =>
          repo.upsert(new FirPlaceholderValueUpsertInput({ firId, placeholderId, value })),
        ),
      );

    await upsert(fir.id, placeholder.id, "پہلا");
    const saved = await upsert(fir.id, placeholder.id, "دوسرا");
    expect(Exit.isSuccess(saved) && saved.value.value).toBe("دوسرا");

    const noFir = await upsert(FirId.make(9999), placeholder.id, "x");
    expect(JSON.stringify(noFir)).toContain('"entity":"fir"');
    const noPlaceholder = await upsert(fir.id, PlaceholderId.make(9999), "x");
    expect(JSON.stringify(noPlaceholder)).toContain('"entity":"placeholder"');
  }));

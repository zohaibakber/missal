/** @vitest-environment jsdom */
import { expect, it, vi } from "vitest";
import { Effect, Schema } from "effect";
import { AtomRegistry, AsyncResult } from "effect/unstable/reactivity";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { makeElectronMainRuntime } from "#/electron/main-runtime";
import { dispatchStorageRequest } from "#/electron/storage-dispatch";
import { createEmptyFirRecord, FirCreateInput, FirRecord, FirUpdateInput } from "#/lib/fir";
import { FirRepository } from "#/repositories/index";
import { atoms } from "#/state/atoms";

it("refreshes mounted document values when FIR details are saved", async () => {
  const directory = mkdtempSync(join(tmpdir(), "missal-fir-refresh-"));
  const runtime = makeElectronMainRuntime({
    databasePath: join(directory, "test.sqlite"),
    migrationsFolder: resolve("drizzle"),
  });
  const registry = AtomRegistry.make();
  window.electronStorage = {
    request: (payload) => runtime.runPromise(dispatchStorageRequest(payload)),
  };
  try {
    const fir = await runtime.runPromise(
      Effect.flatMap(FirRepository, (repository) =>
        repository.create(
          Schema.decodeUnknownSync(FirCreateInput)({
            ...createEmptyFirRecord(),
            fir_no: "BEFORE/26",
            date: "22-09-2026",
            incident_date: "22-09-2026",
            offence: "188",
            accused: ["آزمائشی ملزم"],
          }),
        ),
      ),
    );
    const contextAtom = atoms.firValueContextAtom(fir.id);
    registry.mount(contextAtom);
    await vi.waitFor(() => {
      const result = registry.get(contextAtom);
      expect(AsyncResult.isSuccess(result) && result.value.fir.fir_no).toBe("BEFORE/26");
    });
    registry.set(
      atoms.updateFirAtom,
      Schema.decodeUnknownSync(FirUpdateInput)({
        ...Schema.encodeSync(FirRecord)(fir),
        fir_no: "AFTER/26",
        witness: ["آزمائشی گواہ"],
      }),
    );
    await vi.waitFor(() => {
      const result = registry.get(contextAtom);
      expect(AsyncResult.isSuccess(result) && result.value.fir.fir_no).toBe("AFTER/26");
      expect(AsyncResult.isSuccess(result) && result.value.fir.witness).toEqual(["آزمائشی گواہ"]);
    });
  } finally {
    registry.dispose();
    delete window.electronStorage;
    await runtime.dispose();
    rmSync(directory, { recursive: true, force: true });
  }
});

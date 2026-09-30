import { afterEach, expect, it } from "@effect/vitest";
import { Effect, Exit } from "effect";
import { applyStorageInitFault } from "#/electron/storage-faults";

const previous = {
  MISSAL_FAULTS: process.env.MISSAL_FAULTS,
  MISSAL_PACKAGED: process.env.MISSAL_PACKAGED,
};
afterEach(() => {
  for (const [key, value] of Object.entries(previous)) {
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
});

const run = (faults: string | undefined, packaged: string | undefined) => {
  if (faults === undefined) delete process.env.MISSAL_FAULTS;
  else process.env.MISSAL_FAULTS = faults;
  if (packaged === undefined) delete process.env.MISSAL_PACKAGED;
  else process.env.MISSAL_PACKAGED = packaged;
  return Effect.runPromiseExit(applyStorageInitFault);
};

it("does nothing when no fault is set", async () => {
  expect(Exit.isSuccess(await run(undefined, undefined))).toBe(true);
});

it("ignores injected faults in a packaged app", async () => {
  expect(Exit.isSuccess(await run("storage.failInit", "1"))).toBe(true);
});

it("fails storage initialization when failInit is injected outside a package", async () => {
  expect(Exit.isFailure(await run("storage.slowInit:1,storage.failInit", "0"))).toBe(true);
});

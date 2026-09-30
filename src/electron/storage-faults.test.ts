import { expect, it } from "@effect/vitest";
import { Effect, Exit } from "effect";
import { applyStorageInitFault } from "#/electron/storage-faults";

const KEYS = ["MISSAL_FAULTS", "MISSAL_PACKAGED"] as const;

function restore(previous: Record<string, string | undefined>) {
  for (const key of KEYS) {
    const value = previous[key];
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
}

it("does nothing when MISSAL_FAULTS is unset", async () => {
  const previous = {
    MISSAL_FAULTS: process.env.MISSAL_FAULTS,
    MISSAL_PACKAGED: process.env.MISSAL_PACKAGED,
  };
  delete process.env.MISSAL_FAULTS;
  delete process.env.MISSAL_PACKAGED;
  try {
    const started = Date.now();
    await Effect.runPromise(applyStorageInitFault);
    expect(Date.now() - started).toBeLessThan(200);
  } finally {
    restore(previous);
  }
});

it("ignores injected faults in a packaged app", async () => {
  const previous = {
    MISSAL_FAULTS: process.env.MISSAL_FAULTS,
    MISSAL_PACKAGED: process.env.MISSAL_PACKAGED,
  };
  process.env.MISSAL_PACKAGED = "1";
  process.env.MISSAL_FAULTS = "storage.failInit";
  try {
    await Effect.runPromise(applyStorageInitFault);
  } finally {
    restore(previous);
  }
});

it("fails storage initialization when failInit is injected outside a package", async () => {
  const previous = {
    MISSAL_FAULTS: process.env.MISSAL_FAULTS,
    MISSAL_PACKAGED: process.env.MISSAL_PACKAGED,
  };
  process.env.MISSAL_PACKAGED = "0";
  process.env.MISSAL_FAULTS = "storage.slowInit:1,storage.failInit";
  try {
    const exit = await Effect.runPromiseExit(applyStorageInitFault);
    expect(Exit.isFailure(exit)).toBe(true);
    if (Exit.isFailure(exit)) {
      expect(JSON.stringify(exit.cause)).toContain("storage.init");
    }
  } finally {
    restore(previous);
  }
});

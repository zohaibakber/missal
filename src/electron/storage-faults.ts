import { Duration, Effect } from "effect";
import { StorageError } from "#/lib/storage-errors";

/**
 * Faults apply only outside a packaged app. Main sets MISSAL_PACKAGED from app.isPackaged
 * before the storage worker starts; tests and scripts leave it unset.
 */
function faultsReadable() {
  return process.env.MISSAL_PACKAGED !== "1";
}

export const applyStorageInitFault = Effect.suspend(() => {
  if (!faultsReadable()) return Effect.void;
  const spec = process.env.MISSAL_FAULTS;
  if (!spec) return Effect.void;
  return Effect.gen(function* () {
    for (const part of spec.split(",")) {
      const trimmed = part.trim();
      if (trimmed.length === 0) continue;
      const separator = trimmed.indexOf(":");
      const name = separator === -1 ? trimmed : trimmed.slice(0, separator);
      const argument = separator === -1 ? "" : trimmed.slice(separator + 1);
      if (name === "storage.slowInit") {
        const delayMs = Number(argument);
        if (Number.isFinite(delayMs) && delayMs > 0) yield* Effect.sleep(Duration.millis(delayMs));
      } else if (name === "storage.failInit") {
        return yield* new StorageError({
          message: "Storage initialization failed",
          operation: "storage.init",
        });
      }
    }
  });
});

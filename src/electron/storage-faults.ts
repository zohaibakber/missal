import { Effect } from "effect";
import { StorageError } from "#/lib/storage-errors";

// Main sets MISSAL_PACKAGED from app.isPackaged, which the worker cannot read itself.
export const applyStorageInitFault = Effect.suspend(() => {
  const spec = process.env.MISSAL_FAULTS;
  if (!spec || process.env.MISSAL_PACKAGED === "1") return Effect.void;
  return Effect.forEach(
    spec.split(","),
    (fault) => {
      const [name, argument] = fault.trim().split(":");
      if (name === "storage.slowInit") return Effect.sleep(Number(argument) || 0);
      if (name === "storage.failInit") {
        return Effect.fail(
          new StorageError({ message: "Storage initialization failed", operation: "storage.init" }),
        );
      }
      return Effect.void;
    },
    { discard: true },
  );
});

import { Schema } from "effect";
import { Rpc, RpcGroup } from "effect/unstable/rpc";
import { StorageError } from "#/lib/storage-errors";

export const StorageWorkerConfig = Schema.Struct({
  databasePath: Schema.String,
  migrationsFolder: Schema.String,
});

export type StorageWorkerConfig = typeof StorageWorkerConfig.Type;

export class StorageRpcs extends RpcGroup.make(
  Rpc.make("Storage.open", { error: StorageError }),
  Rpc.make("Storage.request", {
    payload: { payload: Schema.String },
    success: Schema.String,
  }),
) {}

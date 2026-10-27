import { Schema } from "effect";
import { Rpc, RpcGroup } from "effect/rpc";
import { Transferable } from "effect/workers";
import { MessagePort } from "node:worker_threads";
import { StorageError } from "#/lib/storage-errors";

export const StorageWorkerConfig = Schema.Struct({
  databasePath: Schema.String,
  migrationsFolder: Schema.String,
});

export type StorageWorkerConfig = typeof StorageWorkerConfig.Type;

const TransferredPort = Transferable.schema(Schema.instanceOf(MessagePort), (port) => [
  port as unknown as globalThis.Transferable,
]);

export class StorageControlRpcs extends RpcGroup.make(
  Rpc.make("Storage.open", { error: StorageError }),
  Rpc.make("Storage.connect", { payload: { port: TransferredPort }, error: StorageError }),
) {}

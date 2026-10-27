import * as NodeWorker from "@effect/platform-node/NodeWorker";
import { Context, Effect, Layer } from "effect";
import { RpcClient, RpcWorker } from "effect/rpc";
import type { MessagePortMain } from "electron";
import { MessageChannel, type MessagePort, Worker } from "node:worker_threads";
import { StorageControlRpcs, StorageWorkerConfig } from "#/electron/storage-control-rpc";

const pipePorts = (renderer: MessagePortMain, worker: MessagePort) => {
  renderer.on("message", (event) => worker.postMessage(event.data));
  worker.on("message", (data) => renderer.postMessage(data));
  renderer.once("close", () => worker.close());
  worker.once("close", () => renderer.close());
  renderer.start();
};

export class StorageWorker extends Context.Service<
  StorageWorker,
  {
    readonly open: Effect.Effect<void, unknown>;
    readonly connect: (renderer: MessagePortMain) => Effect.Effect<void, unknown>;
  }
>()("missal/StorageWorker") {
  static readonly layer = (options: { workerPath: string; config: StorageWorkerConfig }) =>
    Layer.effect(
      StorageWorker,
      Effect.map(RpcClient.make(StorageControlRpcs), (client) =>
        StorageWorker.of({
          open: client["Storage.open"](),
          connect: (renderer) =>
            Effect.suspend(() => {
              const { port1, port2 } = new MessageChannel();
              pipePorts(renderer, port2);
              return client["Storage.connect"]({
                port: port1,
              });
            }),
        }),
      ),
    ).pipe(
      Layer.provide(RpcClient.layerProtocolWorker({ size: 1 })),
      Layer.provide(NodeWorker.layer(() => new Worker(options.workerPath))),
      Layer.provide(
        RpcWorker.layerInitialMessage(StorageWorkerConfig, Effect.succeed(options.config)),
      ),
    );
}

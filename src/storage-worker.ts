import "effect/schema/SchemaJITCompiler/enable";
import * as NodeRuntime from "@effect/platform-node/NodeRuntime";
import * as NodeWorkerRunner from "@effect/platform-node/NodeWorkerRunner";
import { Effect, Layer } from "effect";
import { RpcServer, RpcWorker } from "effect/rpc";
import { storageControlHandlers } from "#/electron/storage-handlers";
import { StorageControlRpcs, StorageWorkerConfig } from "#/electron/storage-control-rpc";

RpcServer.layer(StorageControlRpcs).pipe(
  Layer.provide(
    Layer.unwrap(
      Effect.map(RpcWorker.initialMessage(StorageWorkerConfig), storageControlHandlers),
    ).pipe(Layer.orDie),
  ),
  Layer.provide(RpcServer.layerProtocolWorkerRunner),
  Layer.provide(NodeWorkerRunner.layer),
  Layer.launch,
  NodeRuntime.runMain,
);

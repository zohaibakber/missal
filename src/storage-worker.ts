// First import: every Schema parser in the worker is JIT-compiled (interpreted where it can't be).
import "effect/unstable/schema/SchemaJITCompiler/enable";
import * as NodeRuntime from "@effect/platform-node/NodeRuntime";
import * as NodeWorkerRunner from "@effect/platform-node/NodeWorkerRunner";
import { Effect, Layer } from "effect";
import { RpcServer, RpcWorker } from "effect/unstable/rpc";
import { storageHandlers } from "#/electron/storage-dispatch";
import { StorageRpcs, StorageWorkerConfig } from "#/electron/storage-rpc";

const HandlersLive = Layer.unwrap(
  Effect.map(RpcWorker.initialMessage(StorageWorkerConfig).pipe(Effect.orDie), storageHandlers),
);

RpcServer.layer(StorageRpcs).pipe(
  Layer.provide(HandlersLive),
  Layer.provide(RpcServer.layerProtocolWorkerRunner),
  Layer.provide(NodeWorkerRunner.layer),
  Layer.launch,
  NodeRuntime.runMain,
);

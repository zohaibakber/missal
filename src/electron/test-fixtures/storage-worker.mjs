import * as NodeRuntime from "@effect/platform-node/NodeRuntime";
import * as NodeWorkerRunner from "@effect/platform-node/NodeWorkerRunner";
import { Effect, Layer, Schema } from "effect";
import { Rpc, RpcGroup, RpcServer } from "effect/rpc";

const rpcs = RpcGroup.make(
  Rpc.make("Storage.open"),
  Rpc.make("Storage.request", { payload: { payload: Schema.String }, success: Schema.String }),
);
const received = [];
const handlers = rpcs.toLayer({
  "Storage.open": () => Effect.void,
  "Storage.request": ({ payload }) => {
    const request = JSON.parse(payload);
    received.push(request._tag);
    return request.block
      ? Effect.never
      : Effect.succeed(JSON.stringify({ _tag: "Success", value: received }));
  },
});

RpcServer.layer(rpcs).pipe(
  Layer.provide(handlers),
  Layer.provide(RpcServer.layerProtocolWorkerRunner),
  Layer.provide(NodeWorkerRunner.layer),
  Layer.launch,
  NodeRuntime.runMain,
);

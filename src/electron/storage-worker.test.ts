import { expect, it } from "@effect/vitest";
import { Cause, Effect, Exit, Schema } from "effect";
import { RpcTest } from "effect/unstable/rpc";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { storageHandlers } from "#/electron/storage-dispatch";
import { StorageRpcs } from "#/electron/storage-rpc";

const databasePath = () => join(mkdtempSync(join(tmpdir(), "missal-worker-")), "test.sqlite");

it.live("opens the database and serves encoded storage requests", () =>
  Effect.gen(function* () {
    const client = yield* RpcTest.makeClient(StorageRpcs);
    yield* client["Storage.open"]();

    expect(yield* client["Storage.request"]({ payload: { _tag: "Fir.list" } })).toEqual({
      _tag: "Success",
      value: [],
    });
    expect(yield* client["Storage.request"]({ payload: { _tag: "Nope" } })).toMatchObject({
      _tag: "Failure",
      error: { _tag: "StorageError", operation: "storage.decode" },
    });
  }).pipe(
    Effect.provide(
      storageHandlers({ databasePath: databasePath(), migrationsFolder: resolve("drizzle") }),
    ),
    Effect.scoped,
  ),
);

it.live("reports a database that cannot be opened", () =>
  Effect.gen(function* () {
    const client = yield* RpcTest.makeClient(StorageRpcs);
    const exit = yield* Effect.exit(client["Storage.open"]());

    expect(Exit.isFailure(exit) && String(Cause.squash(exit.cause))).toContain(
      "/nonexistent/drizzle",
    );
  }).pipe(
    Effect.provide(
      storageHandlers({ databasePath: databasePath(), migrationsFolder: "/nonexistent/drizzle" }),
    ),
    Effect.scoped,
  ),
);

it("carries encoded storage messages with optional fields through the worker codec", () => {
  const rpc = StorageRpcs.requests.get("Storage.request");
  if (!rpc) throw new Error("Missing Storage.request");
  const message = { _tag: "Template.create", input: { name: "x", pageLayout: undefined } };

  expect(
    Schema.encodeUnknownSync(Schema.toCodecJson(rpc.payloadSchema))({ payload: message }),
  ).toEqual({
    payload: message,
  });
  expect(Schema.encodeUnknownSync(Schema.toCodecJson(rpc.successSchema))(message)).toEqual(message);
});

import { expect, it } from "@effect/vitest";
import { Cause, Effect, Exit } from "effect";
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

    const request = (payload: unknown) =>
      client["Storage.request"]({ payload: JSON.stringify(payload) }).pipe(
        Effect.map((response): unknown => JSON.parse(response)),
      );

    expect(yield* request({ _tag: "Fir.list" })).toEqual({ _tag: "Success", value: [] });
    expect(yield* request({ _tag: "Nope" })).toMatchObject({
      _tag: "Failure",
      error: { _tag: "StorageError", operation: "storage.decode" },
    });
    expect(yield* client["Storage.request"]({ payload: "{not json" })).toContain("storage.decode");
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

it.live("answers requests with the reason when the database never opened", () =>
  Effect.gen(function* () {
    const client = yield* RpcTest.makeClient(StorageRpcs);
    const response = yield* client["Storage.request"]({
      payload: JSON.stringify({ _tag: "Fir.list" }),
    });

    expect(JSON.parse(response)).toMatchObject({
      _tag: "Failure",
      error: { _tag: "StorageError" },
    });
  }).pipe(
    Effect.provide(
      storageHandlers({ databasePath: databasePath(), migrationsFolder: "/nonexistent/drizzle" }),
    ),
    Effect.scoped,
  ),
);

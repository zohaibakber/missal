import * as BrowserWorker from "@effect/platform-browser/BrowserWorker";
import { expect, it } from "@effect/vitest";
import { Cause, Effect, Exit, Layer, Schema } from "effect";
import { RpcClient, RpcTest } from "effect/rpc";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { MessageChannel } from "node:worker_threads";
import { storageControlHandlers } from "#/electron/storage-handlers";
import { StorageControlRpcs } from "#/electron/storage-control-rpc";
import { StorageRpcs } from "#/electron/storage-rpc";
import { FirCreateInput, createEmptyFirRecord } from "#/lib/fir";

const databasePath = () => join(mkdtempSync(join(tmpdir(), "missal-worker-")), "test.sqlite");

it.live("serves repository procedures to a renderer over its port", () =>
  Effect.gen(function* () {
    const control = yield* RpcTest.makeClient(StorageControlRpcs);
    yield* control["Storage.open"]();
    const { port1, port2 } = new MessageChannel();
    yield* control["Storage.connect"]({ port: port1 });

    const protocol = yield* Layer.build(
      RpcClient.layerProtocolWorker({ size: 1 }).pipe(
        Layer.provide(BrowserWorker.layer(() => port2 as unknown as MessagePort)),
      ),
    );
    const renderer = yield* RpcClient.make(StorageRpcs).pipe(Effect.provideContext(protocol));
    expect(yield* renderer["Fir.list"]()).toEqual([]);
    const created = yield* renderer["Fir.create"](
      Schema.decodeUnknownSync(FirCreateInput)({
        ...createEmptyFirRecord(),
        fir_no: "12/26",
        date: "12-09-2026",
        incident_date: "12-09-2026",
        offence: "test",
        accused: ["test"],
      }),
    );
    expect(yield* renderer["Fir.get"](created.id)).toEqual(created);
    const missing = yield* Effect.flip(renderer["Fir.get"](created.id + 1));
    expect(missing).toMatchObject({ _tag: "EntityNotFound", entity: "fir" });
  }).pipe(
    Effect.provide(
      storageControlHandlers({
        databasePath: databasePath(),
        migrationsFolder: resolve("drizzle"),
      }),
    ),
    Effect.scoped,
  ),
);

it.live("reports a database that cannot be opened", () =>
  Effect.gen(function* () {
    const control = yield* RpcTest.makeClient(StorageControlRpcs);
    const exit = yield* Effect.exit(control["Storage.open"]());

    expect(Exit.isFailure(exit) && String(Cause.squash(exit.cause))).toContain(
      "/nonexistent/drizzle",
    );
    const { port1 } = new MessageChannel();
    const connected = yield* Effect.exit(control["Storage.connect"]({ port: port1 }));
    expect(Exit.isFailure(connected)).toBe(true);
  }).pipe(
    Effect.provide(
      storageControlHandlers({
        databasePath: databasePath(),
        migrationsFolder: "/nonexistent/drizzle",
      }),
    ),
    Effect.scoped,
  ),
);

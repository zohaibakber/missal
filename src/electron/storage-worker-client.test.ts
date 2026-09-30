import { afterEach, expect, it, vi } from "vitest";
import { resolve } from "node:path";
import { decodeStorageResponse } from "#/electron/storage-contract";
import { Effect } from "effect";
import { startStorageWorker, type StorageHost } from "#/electron/storage-worker-client";

let host: StorageHost | undefined;
afterEach(async () => {
  vi.useRealTimers();
  await host?.dispose();
  host = undefined;
});

async function start() {
  host = startStorageWorker({
    workerPath: resolve("src/electron/test-fixtures/storage-worker.mjs"),
    config: { databasePath: "unused", migrationsFolder: "unused" },
  });
  await host.ready;
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  return host;
}

const response = async (request: Promise<string>) =>
  Effect.runPromise(decodeStorageResponse(await request));

it("expires queued reads and writes without dispatching or reporting an unknown outcome", async () => {
  const host = await start();
  const running = host.request(JSON.stringify({ _tag: "Template.save", block: true }));
  const read = host.request(JSON.stringify({ _tag: "Fir.list" }));
  const write = host.request(JSON.stringify({ _tag: "Settings.save" }));

  await vi.advanceTimersByTimeAsync(10_000);
  expect(await response(read)).toMatchObject({
    _tag: "Failure",
    error: { _tag: "StorageBusy", operation: "Fir.list" },
  });
  await vi.advanceTimersByTimeAsync(5_000);
  expect(await response(write)).toMatchObject({
    _tag: "Failure",
    error: { _tag: "StorageBusy", operation: "Settings.save" },
  });
  await vi.advanceTimersByTimeAsync(15_000);
  expect(await response(running)).toMatchObject({
    _tag: "Failure",
    error: { _tag: "StorageUnknownOutcome", operation: "Template.save" },
  });
  expect(await response(host.request(JSON.stringify({ _tag: "Fir.recent" })))).toEqual({
    _tag: "Success",
    value: ["Template.save", "Fir.recent"],
  });
});

it("reports a dispatched read timeout and releases the permit for the next request", async () => {
  const host = await start();
  const read = host.request(JSON.stringify({ _tag: "Fir.list", block: true }));
  await vi.advanceTimersByTimeAsync(10_000);
  expect(await response(read)).toMatchObject({
    _tag: "Failure",
    error: { _tag: "StorageError", operation: "Fir.list" },
  });
  expect(await response(host.request(JSON.stringify({ _tag: "Fir.recent" })))).toEqual({
    _tag: "Success",
    value: ["Fir.list", "Fir.recent"],
  });
});

import { expect, it } from "vite-plus/test";
import { createPermitPool, mapWithConcurrency } from "#/components/print-preview-schedule";

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

it("keeps page work inside the concurrency limit", async () => {
  let active = 0;
  let max = 0;
  const blocked = gate();
  const done = mapWithConcurrency({
    items: [1, 2, 3, 4, 5, 6],
    concurrency: 3,
    signal: new AbortController().signal,
    worker: async (item) => {
      active += 1;
      max = Math.max(max, active);
      await blocked.promise;
      active -= 1;
      return item;
    },
  });

  await delay(0);
  expect(active).toBe(3);
  expect(max).toBe(3);
  blocked.release();
  await done;
  expect(active).toBe(0);
});

it("does not publish results after a newer run aborts the previous one", async () => {
  const published: number[] = [];
  const controller = new AbortController();
  let started = 0;

  await mapWithConcurrency({
    items: [1, 2, 3, 4],
    concurrency: 1,
    signal: controller.signal,
    worker: async (item) => {
      started += 1;
      await delay(15);
      return item;
    },
    onItem: (_index, value) => {
      published.push(value);
      controller.abort();
    },
  });

  expect(published).toEqual([1]);
  expect(started).toBe(1);
});

it("grants at most the configured canvas permits", async () => {
  const pool = createPermitPool(2);
  let active = 0;
  let max = 0;

  await Promise.all(
    [0, 1, 2, 3].map(async () => {
      const controller = new AbortController();
      await pool.acquire(controller.signal);
      active += 1;
      max = Math.max(max, active);
      await delay(20);
      active -= 1;
      pool.release();
    }),
  );

  expect(max).toBe(2);
  expect(active).toBe(0);
});

it("does not grant a permit to a cancelled waiter", async () => {
  const pool = createPermitPool(1);
  await pool.acquire(new AbortController().signal);
  const cancelled = new AbortController();
  const waiting = pool.acquire(cancelled.signal);
  let granted = false;
  const next = pool.acquire(new AbortController().signal).then(() => {
    granted = true;
  });

  cancelled.abort();
  pool.release();

  await expect(waiting).rejects.toBeTruthy();
  await next;
  expect(granted).toBe(true);
  pool.release();
});

function gate() {
  let release: () => void = () => undefined;
  const promise = new Promise<void>((resolve) => {
    release = resolve;
  });
  return { promise, release };
}

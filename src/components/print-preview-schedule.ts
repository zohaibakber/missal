export const PDF_PAGE_METADATA_CONCURRENCY = 3;
export const PDF_CANVAS_RENDER_CONCURRENCY = 2;

export type PermitPool = ReturnType<typeof createPermitPool>;

/** Caps how many preview canvases rasterize at once. Waiting callers can be cancelled. */
export function createPermitPool(concurrency: number) {
  let active = 0;
  const waiters: Array<() => void> = [];

  return {
    acquire(signal: AbortSignal) {
      return new Promise<void>((resolve, reject) => {
        if (signal.aborted) return reject(signal.reason);
        if (active < concurrency) {
          active += 1;
          return resolve();
        }
        const grant = () => {
          signal.removeEventListener("abort", onAbort);
          active += 1;
          resolve();
        };
        const onAbort = () => {
          waiters.splice(waiters.indexOf(grant), 1);
          reject(signal.reason);
        };
        signal.addEventListener("abort", onAbort, { once: true });
        waiters.push(grant);
      });
    },
    release() {
      active -= 1;
      waiters.shift()?.();
    },
  };
}

/** Runs `worker` for each index with a fixed number of lanes; stops starting work once `signal` aborts. */
export async function forEachWithConcurrency<A>({
  count,
  concurrency,
  signal,
  worker,
  onItem,
}: {
  readonly count: number;
  readonly concurrency: number;
  readonly signal: AbortSignal;
  readonly worker: (index: number) => Promise<A>;
  readonly onItem?: (index: number, value: A) => void;
}) {
  let next = 0;
  const lane = async () => {
    while (!signal.aborted && next < count) {
      const index = next++;
      const value = await worker(index);
      if (!signal.aborted) onItem?.(index, value);
    }
  };
  await Promise.all(Array.from({ length: Math.min(concurrency, count) }, lane));
}

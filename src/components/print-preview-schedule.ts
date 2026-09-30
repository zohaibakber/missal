export const PDF_PAGE_METADATA_CONCURRENCY = 3;
export const PDF_CANVAS_RENDER_CONCURRENCY = 2;

export type PreviewRenderSubscription = {
  cancel: () => void;
};

export type PermitPool = {
  acquire: (signal: AbortSignal) => Promise<void>;
  release: () => void;
};

const abortReason = (signal: AbortSignal) =>
  signal.reason ?? new DOMException("The operation was aborted.", "AbortError");

/** Caps how many preview canvases rasterize at once. Waiting callers can be cancelled. */
export function createPermitPool(concurrency: number): PermitPool {
  const limit = Math.max(1, concurrency);
  let active = 0;
  const waiters: Array<{
    readonly signal: AbortSignal;
    readonly grant: () => void;
  }> = [];

  const grantNext = () => {
    while (active < limit && waiters.length > 0) {
      const waiter = waiters.shift();
      if (!waiter || waiter.signal.aborted) continue;
      waiter.grant();
    }
  };

  return {
    acquire(signal) {
      if (signal.aborted) return Promise.reject(abortReason(signal));
      if (active < limit) {
        active += 1;
        return Promise.resolve();
      }
      return new Promise((resolve, reject) => {
        const waiter = {
          signal,
          grant: () => {
            signal.removeEventListener("abort", onAbort);
            active += 1;
            resolve();
          },
        };
        const onAbort = () => {
          const index = waiters.indexOf(waiter);
          if (index >= 0) waiters.splice(index, 1);
          reject(abortReason(signal));
        };
        signal.addEventListener("abort", onAbort, { once: true });
        waiters.push(waiter);
      });
    },
    release() {
      if (active > 0) active -= 1;
      grantNext();
    },
  };
}

/**
 * Runs `worker` over `items` with a fixed number of slots.
 * After `signal` aborts, no further work starts and late results are not published.
 */
export async function mapWithConcurrency<A, B>({
  items,
  concurrency,
  signal,
  worker,
  onItem,
}: {
  readonly items: readonly A[];
  readonly concurrency: number;
  readonly signal: AbortSignal;
  readonly worker: (item: A, index: number) => Promise<B>;
  readonly onItem?: (index: number, value: B) => void;
}): Promise<void> {
  if (signal.aborted || items.length === 0) return;
  const limit = Math.max(1, concurrency);
  let nextIndex = 0;
  let active = 0;
  let rejected = false;

  await new Promise<void>((resolve, reject) => {
    const finishOne = () => {
      active -= 1;
      if (rejected) return;
      if (signal.aborted || nextIndex >= items.length) {
        if (active === 0) resolve();
        return;
      }
      launch();
    };

    const launch = () => {
      while (!signal.aborted && !rejected && active < limit && nextIndex < items.length) {
        const index = nextIndex;
        const item = items[index];
        if (item === undefined) break;
        nextIndex += 1;
        active += 1;
        Promise.resolve()
          .then(() => worker(item, index))
          .then(
            (value) => {
              if (!signal.aborted && !rejected) onItem?.(index, value);
              finishOne();
            },
            (error: unknown) => {
              if (signal.aborted) {
                finishOne();
                return;
              }
              rejected = true;
              active -= 1;
              reject(error instanceof Error ? error : new Error("Preview work failed"));
            },
          );
      }
    };

    launch();
  });
}

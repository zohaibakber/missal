export { cn } from "cn";

/**
 * Flags `pending` for the duration of `task`. Keeps `try`/`finally` out of components, which
 * React Compiler can't compile.
 */
export async function whilePending<A>(
  setPending: (pending: boolean) => void,
  task: () => Promise<A>,
): Promise<A> {
  setPending(true);
  try {
    return await task();
  } finally {
    setPending(false);
  }
}

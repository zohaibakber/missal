export { cn } from "cn";

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

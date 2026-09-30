import { defaultFilter } from "cmdk";

export const COMMAND_MATCH_LIMIT = 20;

export function topCommandMatches<T>(
  items: readonly T[],
  search: string,
  value: (item: T) => string,
  limit = COMMAND_MATCH_LIMIT,
): T[] {
  if (!search) return items.slice(0, limit);

  const top: { readonly item: T; readonly score: number }[] = [];
  for (const item of items) {
    const score = defaultFilter(value(item), search);
    if (score <= 0) continue;
    const worst = top.at(-1);
    if (top.length === limit && worst && score <= worst.score) continue;
    // Insert after equal scores so ties keep list order, as a stable sort would.
    let at = top.length;
    while (at > 0 && (top[at - 1]?.score ?? score) < score) at -= 1;
    top.splice(at, 0, { item, score });
    if (top.length > limit) top.pop();
  }
  return top.map((match) => match.item);
}

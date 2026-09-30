import { defaultFilter } from "cmdk";

export const COMMAND_MATCH_LIMIT = 20;

type Ranked<T> = {
  readonly index: number;
  readonly item: T;
  readonly score: number;
};

// Higher score first. Equal scores keep earlier items, matching a stable sort.
function compareRank<T>(left: Ranked<T>, right: Ranked<T>) {
  if (left.score !== right.score) return right.score - left.score;
  return left.index - right.index;
}

function insertByRank<T>(selected: Ranked<T>[], candidate: Ranked<T>) {
  let index = selected.length;
  while (index > 0) {
    const previous = selected[index - 1];
    if (!previous || compareRank(candidate, previous) >= 0) break;
    index -= 1;
  }
  selected.splice(index, 0, candidate);
}

export function topCommandMatches<T>(
  items: readonly T[],
  search: string,
  value: (item: T) => string,
  limit = COMMAND_MATCH_LIMIT,
): T[] {
  if (!search) return items.slice(0, limit);

  const selected: Ranked<T>[] = [];
  for (let index = 0; index < items.length; index++) {
    const item = items[index];
    if (item === undefined) continue;
    const score = defaultFilter(value(item), search);
    if (!(score > 0)) continue;
    const candidate = { index, item, score };
    if (selected.length < limit) {
      insertByRank(selected, candidate);
      continue;
    }
    const worst = selected[selected.length - 1];
    if (worst && compareRank(candidate, worst) < 0) {
      selected.pop();
      insertByRank(selected, candidate);
    }
  }
  return selected.map((entry) => entry.item);
}

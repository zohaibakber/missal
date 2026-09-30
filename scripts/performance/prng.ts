/** mulberry32. Same seed returns the same sequence. */
export function mulberry32(seed: number) {
  let state = seed >>> 0;
  return function next() {
    state = (state + 0x6d2b79f5) | 0;
    let mixed = Math.imul(state ^ (state >>> 15), 1 | state);
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed;
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

export function pickInt(random: () => number, min: number, maxInclusive: number) {
  const span = maxInclusive - min + 1;
  return min + Math.floor(random() * span);
}

export function pickItem<T>(random: () => number, items: readonly T[]) {
  return items[Math.floor(random() * items.length)] as T;
}

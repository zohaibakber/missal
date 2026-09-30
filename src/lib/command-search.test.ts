import { expect, it } from "@effect/vitest";
import { defaultFilter } from "cmdk";
import { COMMAND_MATCH_LIMIT, topCommandMatches } from "#/lib/command-search";

type Named = { readonly id: number; readonly name: string };

const valueOf = (item: Named) => `template ${item.name} ${item.id}`;

function sortAllThenSlice(items: readonly Named[], search: string, limit: number) {
  if (!search) return items.slice(0, limit);
  return items
    .map((item) => ({ item, score: defaultFilter(valueOf(item), search) }))
    .filter((match) => match.score > 0)
    .toSorted((first, second) => second.score - first.score)
    .slice(0, limit)
    .map((match) => match.item);
}

const urdu = ["چالان", "تفتیش", "مقدمہ", "گواہ", "ملزم", "تھانہ", "تاریخ", "زیر دفعہ"];
const english = ["challan", "inquiry", "witness", "accused", "station", "report", "draft", "note"];

const items: Named[] = [];
for (let index = 0; index < 48; index++) {
  const urduName = urdu[index % urdu.length] ?? "چالان";
  const englishName = english[index % english.length] ?? "note";
  items.push({ id: index + 1, name: `${urduName} ${englishName} ${index}` });
}
items.push(
  { id: 100, name: "چالان عدالت" },
  { id: 101, name: "چالان عدالت" },
  { id: 102, name: "چالان عدالت" },
  { id: 103, name: "Challan report عدالت" },
  { id: 104, name: "غیر متعلقہ دستاویز" },
);

const queries = [
  "",
  "چال",
  "چالان",
  "تفتیش",
  "عدالت",
  "challan",
  "report",
  "م",
  "a",
  "1",
  "xyz",
  "گواہ",
];

it("keeps the same order as sorting every match and taking twenty", () => {
  for (const query of queries) {
    expect(topCommandMatches(items, query, valueOf)).toEqual(
      sortAllThenSlice(items, query, COMMAND_MATCH_LIMIT),
    );
  }
});

it("keeps stable ties and a shorter limit in the same order", () => {
  const tied = items.filter((item) => item.name.startsWith("چالان"));
  expect(tied.length).toBeGreaterThan(5);
  for (const limit of [1, 3, 5, 20, 100]) {
    expect(topCommandMatches(tied, "چالان عدالت", valueOf, limit)).toEqual(
      sortAllThenSlice(tied, "چالان عدالت", limit),
    );
    expect(topCommandMatches(items, "عدالت", valueOf, limit).length).toBeLessThanOrEqual(limit);
  }
});

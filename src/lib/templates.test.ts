import { expect, it } from "@effect/vitest";
import { PlaceholderId } from "#/lib/ids";
import {
  createDefaultPlaceholders,
  indexPlaceholders,
  resolveFieldReference,
  resolvePlaceholder,
} from "#/lib/placeholder";

const catalog = createDefaultPlaceholders();
const index = indexPlaceholders(catalog);

it("indexes seeded fields by name alone", () => {
  expect(resolvePlaceholder("ایف آئی آر نمبر", index)?.id).toBe(PlaceholderId.make(1));
  expect(resolvePlaceholder("  ایف آئی آر   نمبر ", index)?.id).toBe(PlaceholderId.make(1));
  expect(resolvePlaceholder("fir_no", index)).toBeUndefined();
  expect(resolvePlaceholder("1", index)).toBeUndefined();
});

it("resolves typed names to catalog field references", () => {
  expect(resolveFieldReference("ایف آئی آر نمبر", index)).toEqual({
    _tag: "CatalogField",
    id: PlaceholderId.make(1),
  });
  expect(resolveFieldReference("unknown", index)).toEqual({
    _tag: "UnresolvedToken",
    text: "unknown",
  });
});

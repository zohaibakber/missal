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
  expect(resolvePlaceholder("مقدمہ نمبر", index)?.id).toBe(PlaceholderId.make(1));
  expect(resolvePlaceholder("  مقدمہ   نمبر ", index)?.id).toBe(PlaceholderId.make(1));
  expect(resolvePlaceholder("fir_no", index)).toBeUndefined();
  expect(resolvePlaceholder("1", index)).toBeUndefined();
});

it("resolves typed names to catalog field references", () => {
  expect(resolveFieldReference("مقدمہ نمبر", index)).toEqual({
    _tag: "CatalogField",
    id: PlaceholderId.make(1),
  });
  expect(resolveFieldReference("unknown", index)).toEqual({
    _tag: "UnresolvedToken",
    text: "unknown",
  });
});

it("matches Word merge-field spellings of names", () => {
  const idOf = (label: string) => catalog.find((field) => field.label === label)?.id;
  expect(resolvePlaceholder("تھانہ_نام_", index)?.id).toBe(idOf("تھانہ نام"));
  expect(resolvePlaceholder("Date_FIR", index)?.id).toBe(idOf("Date FIR"));
  expect(resolvePlaceholder("گواہان__1", index)?.id).toBe(idOf("گواہان 1"));
  expect(resolvePlaceholder("گواہان2", index)?.id).toBe(idOf("گواہان 2"));
  expect(resolvePlaceholder("تاریخ2", index)?.id).toBe(idOf("تاریخ2"));
});

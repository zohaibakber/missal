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

it("indexes seeded fields by key and id", () => {
  expect(resolvePlaceholder("fir_no", index)?.id).toBe(PlaceholderId.make(1));
  expect(resolvePlaceholder("1", index)?.key).toBe("fir_no");
  expect(catalog.find((field) => field.key === "nic")?.source._tag).toBe("FirProperty");
});

it("resolves typed tokens to catalog field references", () => {
  expect(resolveFieldReference("fir_no", index)).toEqual({
    _tag: "CatalogField",
    id: PlaceholderId.make(1),
  });
  expect(resolveFieldReference("unknown", index)).toEqual({
    _tag: "UnresolvedToken",
    text: "unknown",
  });
});

import { expect, it } from "@effect/vitest";
import { Schema } from "effect";
import { encodeStorageResponse, StorageResponse } from "#/electron/storage-contract";
import { FirPropertySource } from "#/lib/field";

it("preserves storage responses across Electron structured cloning", () => {
  const encoded = encodeStorageResponse({
    _tag: "Success",
    value: [
      {
        id: 1,
        key: "fir_no",
        label: "ایف آئی آر نمبر",
        source: FirPropertySource.make({ property: "fir_no" }),
      },
    ],
  });
  const cloned = structuredClone(encoded);
  const decoded = Schema.decodeUnknownSync(StorageResponse)(cloned);

  expect(decoded).toEqual({
    _tag: "Success",
    value: [
      {
        id: 1,
        key: "fir_no",
        label: "ایف آئی آر نمبر",
        source: { _tag: "FirProperty", property: "fir_no" },
      },
    ],
  });
});

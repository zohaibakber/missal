import { expect, it } from "@effect/vitest";
import { Effect } from "effect";
import { decodeStorageResponse, encodeStorageResponse } from "#/electron/storage-contract";

it.effect("keeps an empty success, whose value JSON drops", () =>
  Effect.gen(function* () {
    const encoded = encodeStorageResponse({ _tag: "Success", value: undefined });
    expect(yield* decodeStorageResponse(encoded)).toMatchObject({ _tag: "Success" });
  }),
);

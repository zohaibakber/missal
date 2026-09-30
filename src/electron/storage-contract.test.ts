import { expect, it } from "@effect/vitest";
import { Effect, Schema } from "effect";
import {
  decodeStorageResponse,
  encodeStorageResponse,
  FirRecentRequest,
} from "#/electron/storage-contract";

it("accepts a recent-FIR limit from 1 to 50", () => {
  const decode = (limit: number) =>
    Schema.decodeUnknownExit(FirRecentRequest)({ _tag: "Fir.recent", limit })._tag;
  expect([1, 50, 0, 51].map(decode)).toEqual(["Success", "Success", "Failure", "Failure"]);
});

it.effect("keeps an empty success, whose value JSON drops", () =>
  Effect.gen(function* () {
    const encoded = encodeStorageResponse({ _tag: "Success", value: undefined });
    expect(yield* decodeStorageResponse(encoded)).toMatchObject({ _tag: "Success" });
  }),
);

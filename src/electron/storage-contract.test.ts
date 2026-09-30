import { expect, it } from "@effect/vitest";
import { Effect, Schema } from "effect";
import {
  decodeStorageResponse,
  encodeStorageResponse,
  FirRecentRequest,
} from "#/electron/storage-contract";

it("accepts a recent-FIR limit from 1 to 50", () => {
  expect(Schema.decodeUnknownExit(FirRecentRequest)({ _tag: "Fir.recent", limit: 1 })._tag).toBe(
    "Success",
  );
  expect(Schema.decodeUnknownExit(FirRecentRequest)({ _tag: "Fir.recent", limit: 50 })._tag).toBe(
    "Success",
  );
  expect(Schema.decodeUnknownExit(FirRecentRequest)({ _tag: "Fir.recent", limit: 0 })._tag).toBe(
    "Failure",
  );
  expect(Schema.decodeUnknownExit(FirRecentRequest)({ _tag: "Fir.recent", limit: 51 })._tag).toBe(
    "Failure",
  );
});

it.effect("keeps an empty success, whose value JSON drops", () =>
  Effect.gen(function* () {
    const encoded = encodeStorageResponse({ _tag: "Success", value: undefined });
    expect(yield* decodeStorageResponse(encoded)).toMatchObject({ _tag: "Success" });
  }),
);

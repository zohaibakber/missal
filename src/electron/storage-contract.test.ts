import { expect, it } from "@effect/vitest";
import { Effect } from "effect";
import {
  decodeStorageRequest,
  decodeStorageResponse,
  encodeStorageRequest,
  encodeStorageResponse,
} from "#/electron/storage-contract";
import { emptyDocumentEnvelope } from "#/lib/document-format";
import { FirPropertySource } from "#/lib/field";
import { TemplateCreateInput } from "#/lib/templates";

it.effect("round-trips storage responses through JSON text", () =>
  Effect.gen(function* () {
    const encoded = encodeStorageResponse({
      _tag: "Success",
      value: [
        {
          id: 1,
          label: "ایف آئی آر نمبر",
          source: FirPropertySource.make({ property: "fir_no" }),
        },
      ],
    });

    expect(typeof encoded).toBe("string");
    expect(yield* decodeStorageResponse(encoded)).toEqual({
      _tag: "Success",
      value: [
        {
          id: 1,
          label: "ایف آئی آر نمبر",
          source: { _tag: "FirProperty", property: "fir_no" },
        },
      ],
    });
  }),
);

it.effect("keeps an empty success, whose value JSON drops", () =>
  Effect.gen(function* () {
    const encoded = encodeStorageResponse({ _tag: "Success", value: undefined });
    expect(yield* decodeStorageResponse(encoded)).toMatchObject({ _tag: "Success" });
  }),
);

it.effect("round-trips requests whose optional fields are absent", () =>
  Effect.gen(function* () {
    const input = new TemplateCreateInput({ document: emptyDocumentEnvelope(), name: "FIR" });
    const decoded = yield* decodeStorageRequest(
      encodeStorageRequest({ _tag: "Template.create", input }),
    );

    expect(decoded).toEqual({ _tag: "Template.create", input });
  }),
);

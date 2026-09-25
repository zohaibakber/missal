import { expect, it } from "@effect/vitest";
import { DocumentEnvelope, documentWriteColumns, projectDocument } from "#/lib/document-format";
import {
  catalogFieldPresentation,
  FieldOverride,
  fieldDisplayText,
  fieldSourceForSeedKey,
  resolveFieldValue,
} from "#/lib/field";
import { FirRecord } from "#/lib/fir";
import { FirId, PlaceholderId } from "#/lib/ids";
import { createDefaultPlaceholders, indexPlaceholders } from "#/lib/placeholder";
import { printPacketFromSections } from "#/lib/output";

const catalog = createDefaultPlaceholders();
const fir = new FirRecord({
  NIC: "12345-1234567-1",
  accused: ["accused"],
  arrest_date: "",
  date: "02-01-2026",
  fir_no: "42",
  id: FirId.make(1),
  incident_date: "01-01-2026",
  investigation_officer: "",
  mobile: "",
  offence: "theft",
  status: "Open",
  witness: [],
  zimni: [],
});

it("binds seeded keys independently of later key names", () => {
  expect(fieldSourceForSeedKey("fir_no")).toEqual({
    _tag: "FirProperty",
    property: "fir_no",
  });
  expect(fieldSourceForSeedKey("police_station")).toEqual({
    _tag: "SharedSetting",
    setting: "police_station",
  });
  expect(fieldSourceForSeedKey("custom")).toEqual({ _tag: "Custom" });
});

it("resolves FIR properties and uses global values even when legacy FIR overrides exist", () => {
  const policeStation = catalog.find((field) => field.label === "تھانہ نام");
  const district = catalog.find((field) => field.label === "ضلع نام");
  const firNo = catalog.find((field) => field.label === "ایف آئی آر نمبر");

  expect(policeStation && district && firNo).toBeTruthy();
  if (!policeStation || !district || !firNo) {
    return;
  }

  const overrides = [
    new FieldOverride({
      placeholderId: PlaceholderId.make(1),
      value: "should-not-win",
    }),
    new FieldOverride({
      placeholderId: policeStation.id,
      value: "station-from-fir",
    }),
  ];
  const sharedSettings = {
    district: "Lahore",
    police_station: "global-station",
  };

  expect(resolveFieldValue(firNo, fir, overrides, sharedSettings)).toEqual({
    _tag: "Resolved",
    text: "42",
  });
  expect(resolveFieldValue(policeStation, fir, overrides, sharedSettings)).toEqual({
    _tag: "Resolved",
    text: "global-station",
  });
  expect(resolveFieldValue(district, fir, overrides, sharedSettings)).toEqual({
    _tag: "Resolved",
    text: "Lahore",
  });
});

it("binds seeded fields to their source, not their name", () => {
  const nic = catalog.find((field) => field.label === "شناختی کارڈ");
  expect(nic?.source).toEqual({ _tag: "FirProperty", property: "NIC" });
  expect(nic ? resolveFieldValue(nic, fir, [], {}) : undefined).toEqual({
    _tag: "Resolved",
    text: "12345-1234567-1",
  });
});

it("projects field references from a serialized envelope without walking live editor state", () => {
  const envelope = new DocumentEnvelope({
    format: "missal-lexical",
    state: {
      root: {
        children: [
          {
            children: [
              { text: "FIR ", type: "text", version: 1 },
              {
                reference: { _tag: "CatalogField", id: 1 },
                type: "field",
                version: 1,
              },
            ],
            type: "paragraph",
            version: 1,
          },
        ],
        type: "root",
        version: 1,
      },
    },
    version: 1,
  });

  const projections = projectDocument(envelope);
  expect(projections.plainText).toBe("FIR");
  expect(projections.fieldCount).toBe(1);
  expect(projections.fieldReferences[0]).toEqual({ _tag: "CatalogField", id: 1 });
  expect(documentWriteColumns(envelope)).toMatchObject({
    fieldCount: 1,
    plainText: "FIR",
    previewText: "FIR",
  });
});

it("shows catalog labels without a FIR and values when a FIR is present", () => {
  const index = indexPlaceholders(catalog);
  const firNo = catalog.find((field) => field.label === "ایف آئی آر نمبر");
  expect(firNo).toBeTruthy();
  if (!firNo) {
    return;
  }

  const reference = { _tag: "CatalogField" as const, id: firNo.id };
  const labels = catalogFieldPresentation(index, "labels");
  expect(fieldDisplayText(reference, labels)).toEqual({
    text: "ایف آئی آر نمبر",
    unresolved: false,
  });

  const values = {
    ...labels,
    displayMode: "values" as const,
    fir,
  };
  expect(fieldDisplayText(reference, values)).toEqual({
    text: "42",
    unresolved: false,
  });
  expect(fieldDisplayText({ _tag: "UnresolvedToken", text: "unknown" }, values)).toEqual({
    text: "unknown",
    unresolved: true,
  });
});

it("builds a print packet without walking a live editor", () => {
  const html = printPacketFromSections(
    [
      { _tag: "Html", html: "<p>one</p>" },
      { _tag: "Preview", previewText: "two" },
    ],
    "FIR 42",
  );
  expect(html).toContain("FIR 42");
  expect(html).toContain("one");
  expect(html).toContain("two");
  expect(html).toContain("missal-print-break");
  expect(html).toContain("missal-page-break");
});

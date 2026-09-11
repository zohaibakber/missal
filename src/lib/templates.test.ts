import { expect, it } from "@effect/vitest";
import { FirRecord } from "#/lib/fir";
import { FirId, PlaceholderId } from "#/lib/ids";
import { createDefaultPlaceholders, indexPlaceholders } from "#/lib/placeholder";
import { DEFAULT_SETTINGS_UPDATED_AT } from "#/lib/settings";
import {
  buildTemplateValues,
  extractPlaceholders,
  FirPlaceholderValue,
  renderTemplate,
} from "#/lib/templates";

const catalog = createDefaultPlaceholders();
const index = indexPlaceholders(catalog);

const fir = new FirRecord({
  id: FirId.make(1),
  fir_no: "42",
  date: "02-01-2026",
  offence: "theft",
  accused: "accused",
  witness: "",
  NIC: "12345-1234567-1",
  mobile: "",
  incident_date: "01-01-2026",
  arrest_date: "",
  investigation_officer: "",
  status: "Open",
  content: "",
});

it("extracts numeric and key tokens", () => {
  expect(extractPlaceholders("FIR @1@ @nic@ @missing@", index)).toEqual(["fir_no", "nic"]);
});

it("prefers core fields, then FIR values, then shared settings", () => {
  const extraValues = [
    new FirPlaceholderValue({
      firId: fir.id,
      placeholderId: PlaceholderId.make(1),
      updatedAt: DEFAULT_SETTINGS_UPDATED_AT,
      value: "should-not-win",
    }),
    new FirPlaceholderValue({
      firId: fir.id,
      placeholderId: PlaceholderId.make(11),
      updatedAt: DEFAULT_SETTINGS_UPDATED_AT,
      value: "station-from-fir",
    }),
  ];
  const values = buildTemplateValues({
    catalog: index,
    extraValues,
    fir,
    placeholders: ["fir_no", "nic", "police_station", "district"],
    sharedValues: {
      police_station: "should-not-win",
      district: "Lahore",
    },
  });

  expect(values.fir_no).toBe("42");
  expect(values.nic).toBe("12345-1234567-1");
  expect(values.police_station).toBe("station-from-fir");
  expect(values.district).toBe("Lahore");
  expect(renderTemplate("No. @fir_no@ @police_station@ @unknown@", values, index)).toBe(
    "No. 42 station-from-fir @unknown@",
  );
});

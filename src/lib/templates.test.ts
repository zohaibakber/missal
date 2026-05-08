import { describe, expect, test } from "vite-plus/test";
import type { FirRecord } from "#/lib/fir";
import {
  buildTemplateValues,
  extractPlaceholders,
  getMissingPlaceholders,
  renderTemplateHtml,
  renderTemplate,
} from "#/lib/templates";
import { buildSharedPlaceholderValues } from "#/lib/settings";

const fir: FirRecord = {
  id: 1,
  fir_no: "23/26",
  date: "13.01.2026",
  offence: "411/379",
  accused: "آصف عرف کوجی",
  witness: "علی حسین",
  NIC: "3520288701547",
  mobile: "03001234567",
  incident_date: "12.02.2026",
  status: "Open",
};

describe("template placeholders", () => {
  test("extracts mixed Urdu and English placeholders once", () => {
    expect(extractPlaceholders("«Date_FIR» «جرم_» «Date_FIR» «تھانہ_نام_»")).toEqual([
      "Date_FIR",
      "جرم_",
      "تھانہ_نام_",
    ]);
  });

  test("renders repeated placeholders", () => {
    const content = "FIR «Date_FIR» / «Date_FIR» جرم «جرم_»";
    const placeholders = extractPlaceholders(content);
    const values = buildTemplateValues({
      extraValues: [],
      fir,
      placeholders,
    });

    expect(renderTemplate(content, values)).toBe("FIR 13.01.2026 / 13.01.2026 جرم 411/379");
  });

  test("leaves unresolved placeholders visible", () => {
    const content = "تھانہ «تھانہ_نام_»";
    const placeholders = extractPlaceholders(content);
    const values = buildTemplateValues({
      extraValues: [],
      fir,
      placeholders,
    });

    expect(renderTemplate(content, values)).toBe("تھانہ «تھانہ_نام_»");
    expect(getMissingPlaceholders(placeholders, values)).toEqual(["تھانہ_نام_"]);
  });

  test("applies FIR core values before extra values", () => {
    const content = "FIR «مقدمہ_نمبر» تھانہ «تھانہ_نام_»";
    const placeholders = extractPlaceholders(content);
    const values = buildTemplateValues({
      extraValues: [
        {
          id: "1:مقدمہ_نمبر",
          firId: 1,
          placeholder: "مقدمہ_نمبر",
          value: "wrong",
          updatedAt: "2026-01-01T00:00:00.000Z",
        },
        {
          id: "1:تھانہ_نام_",
          firId: 1,
          placeholder: "تھانہ_نام_",
          value: "تھانہ شادمان",
          updatedAt: "2026-01-01T00:00:00.000Z",
        },
      ],
      fir,
      placeholders,
    });

    expect(renderTemplate(content, values)).toBe("FIR 23/26 تھانہ تھانہ شادمان");
  });

  test("uses shared settings values for unresolved placeholders", () => {
    const content = "تھانہ «تھانہ_نام_» ضلع «ضلع_نام_»";
    const placeholders = extractPlaceholders(content);
    const values = buildTemplateValues({
      extraValues: [],
      fir,
      placeholders,
      sharedValues: buildSharedPlaceholderValues({
        policeStation: "تھانہ سٹی",
        district: "لاہور",
      }),
    });

    expect(renderTemplate(content, values)).toBe("تھانہ تھانہ سٹی ضلع لاہور");
  });

  test("escapes inserted values when rendering html", () => {
    const content = "<p>«تھانہ_نام_»</p>";
    const values = {
      تھانہ_نام_: "<script>alert(1)</script>\nتھانہ شادمان",
    };

    expect(renderTemplateHtml(content, values)).toBe(
      "<p>&lt;script&gt;alert(1)&lt;/script&gt;<br>تھانہ شادمان</p>",
    );
  });
});

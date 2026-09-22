import { expect, it } from "vite-plus/test";
import { PageLayout } from "#/lib/document-format";
import { printPacketFromSections } from "#/lib/output";

it("prints mixed paper sizes with each document's imported margins", () => {
  const html = printPacketFromSections(
    [
      {
        _tag: "Html",
        html: '<p style="margin: 0; text-align: center; line-height: 1.5">چالان</p>',
        pageLayout: new PageLayout({
          widthMm: 215.9,
          heightMm: 355.6,
          marginTopMm: 12.7,
          marginRightMm: 20,
          marginBottomMm: 15,
          marginLeftMm: 25.4,
        }),
      },
      {
        _tag: "Html",
        html: "<p>اگلا صفحہ</p>",
        pageLayout: new PageLayout({
          widthMm: 210,
          heightMm: 297,
          marginTopMm: 18,
          marginRightMm: 18,
          marginBottomMm: 18,
          marginLeftMm: 18,
        }),
      },
    ],
    "چالان",
  );

  expect(html).toContain(
    "@page missalDocument0 { size: 215.9mm 355.6mm; margin: 12.7mm 0 15mm 0; }",
  );
  expect(html).toContain("@page missalDocument1 { size: 210mm 297mm; margin: 18mm 0 18mm 0; }");
  // Side margins are section padding so content indented into the margin isn't clipped.
  expect(html).toContain(
    'class="missal-print-document" style="page: missalDocument0; padding: 0 20mm 0 25.4mm"',
  );
  expect(html).toContain(
    'class="missal-print-document missal-print-break" style="page: missalDocument1; padding: 0 18mm 0 18mm"',
  );
  expect(html).toContain('<p style="margin: 0; text-align: center; line-height: 1.5">چالان</p>');
});

it("preserves table widths, borders, spanning cells and explicit page breaks in a packet", () => {
  const table =
    '<table style="width: 160mm; table-layout: fixed"><colgroup><col style="width: 30mm"><col style="width: 130mm"></colgroup><tbody><tr><td rowspan="2" style="border: none; padding: 0">گواہ</td><td style="border: 2px double black">نام</td></tr><tr><td>پتہ</td></tr></tbody></table>';
  const pageBreak = '<hr class="missal-page-break">';
  const html = printPacketFromSections(
    [
      { _tag: "Html", html: table + pageBreak + "<p>اگلا صفحہ</p>" },
      { _tag: "Preview", previewText: "<script>unsafe</script>" },
    ],
    "<چالان>",
  );

  expect(html).toContain(table + pageBreak);
  expect(html).toContain("&lt;script&gt;unsafe&lt;/script&gt;");
  expect(html).toContain("<title>&lt;چالان&gt;</title>");
  expect(html).toContain("@page { size: A4; margin: 18mm 0; }");
});

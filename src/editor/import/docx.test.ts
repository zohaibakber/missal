/** @vitest-environment jsdom */
import { expect, it } from "vitest";
import JSZip from "jszip";
import { importDocx, normalizeComplexScriptFormatting } from "#/editor/import/docx";

const NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

it("uses complex-script font and size for Urdu runs without changing Latin runs", () => {
  const xml = `<w:document xmlns:w="${NS}"><w:body><w:p>
    <w:r><w:rPr><w:rtl/><w:rFonts w:ascii="Arial" w:cs="Jameel Noori Nastaleeq"/><w:sz w:val="22"/><w:szCs w:val="32"/><w:bCs/></w:rPr><w:t>ملزم</w:t></w:r>
    <w:r><w:rPr><w:sz w:val="22"/><w:szCs w:val="32"/></w:rPr><w:t>Latin</w:t></w:r>
  </w:p></w:body></w:document>`;
  const parsed = new DOMParser().parseFromString(
    normalizeComplexScriptFormatting(xml),
    "application/xml",
  );
  const runs = parsed.getElementsByTagNameNS(NS, "rPr");
  expect(runs[0]?.getElementsByTagNameNS(NS, "sz")[0]?.getAttributeNS(NS, "val")).toBe("32");
  expect(runs[0]?.getElementsByTagNameNS(NS, "rFonts")[0]?.getAttributeNS(NS, "ascii")).toBe(
    "Jameel Noori Nastaleeq",
  );
  expect(runs[0]?.getElementsByTagNameNS(NS, "b")).toHaveLength(1);
  expect(runs[1]?.getElementsByTagNameNS(NS, "sz")[0]?.getAttributeNS(NS, "val")).toBe("22");
  expect(parsed.documentElement.textContent).toContain("ملزم");
});

it("honors explicit disabled RTL and complex-script bold flags", () => {
  const xml = `<w:document xmlns:w="${NS}"><w:body><w:p>
    <w:r><w:rPr><w:rtl w:val="0"/><w:sz w:val="22"/><w:szCs w:val="32"/></w:rPr></w:r>
    <w:r><w:rPr><w:rtl/><w:b/><w:bCs w:val="0"/></w:rPr></w:r>
  </w:p></w:body></w:document>`;
  const parsed = new DOMParser().parseFromString(
    normalizeComplexScriptFormatting(xml),
    "application/xml",
  );
  const runs = parsed.getElementsByTagNameNS(NS, "rPr");
  expect(runs[0]?.getElementsByTagNameNS(NS, "sz")[0]?.getAttributeNS(NS, "val")).toBe("22");
  expect(runs[1]?.getElementsByTagNameNS(NS, "b")[0]?.getAttributeNS(NS, "val")).toBe("0");
});

it("imports page geometry, RTL table content, and explicit page breaks from a DOCX archive", async () => {
  const archive = new JSZip();
  archive.file(
    "[Content_Types].xml",
    `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>`,
  );
  archive.file(
    "_rels/.rels",
    `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>`,
  );
  archive.file(
    "word/document.xml",
    `<w:document xmlns:w="${NS}"><w:body>
    <w:p><w:pPr><w:bidi/><w:spacing w:after="120"/></w:pPr><w:r><w:rPr><w:rtl/><w:szCs w:val="32"/></w:rPr><w:t>نام {{ملزم}}</w:t></w:r></w:p>
    <w:tbl><w:tblPr><w:tblW w:w="4000" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>گواہ</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
    <w:p><w:r><w:br w:type="page"/></w:r></w:p>
    <w:p><w:r><w:t>اگلا صفحہ</w:t></w:r></w:p>
    <w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="720" w:right="900" w:bottom="720" w:left="900"/></w:sectPr>
  </w:body></w:document>`,
  );
  const buffer = await archive.generateAsync({ type: "arraybuffer" });
  const file = new File([buffer], "example.docx");
  // jsdom's File lacks Blob.arrayBuffer; provide its real bytes for the browser boundary.
  file.arrayBuffer = async () => buffer;
  const result = await importDocx(file);
  const dom = new DOMParser().parseFromString(result.html, "text/html");
  expect(result.pageLayout.widthMm).toBeCloseTo(210, 0);
  expect(result.pageLayout.marginTopMm).toBeCloseTo(12.7, 2);
  expect(result.pageLayout.marginRightMm).toBeCloseTo(15.875, 2);
  expect(dom.querySelectorAll("table")).toHaveLength(1);
  expect(dom.querySelectorAll("[data-page-break]")).toHaveLength(1);
  expect(dom.querySelector("p")?.dir).toBe("rtl");
  expect(dom.querySelector("span")?.style.fontSize).toBe("21.3333px");
  expect(dom.body.textContent).toContain("{{ملزم}}");
  expect(dom.body.textContent).toContain("اگلا صفحہ");
  expect(document.querySelector("iframe")).toBeNull();
});

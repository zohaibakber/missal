/** @vitest-environment jsdom */
import { it, expect } from "vitest";
import { readFileSync, writeFileSync } from "node:fs";
import { importDocx } from "./docx";
it("checks the supplied source DOCX", async () => {
  const bytes = readFileSync(
    "/home/madmax/Downloads/چالان آپریشن بغیر فرد ملزم جوڈیشل جنرل 1 گواہ (1).docx",
  );
  const buffer = Uint8Array.from(bytes).buffer;
  const file = new File([buffer], "source.docx");
  file.arrayBuffer = async () => buffer;
  const result = await importDocx(file);
  const doc = new DOMParser().parseFromString(result.html, "text/html");
  const evidence = {
    pageLayout: result.pageLayout,
    notices: result.notices,
    tables: doc.querySelectorAll("table").length,
    breaks: doc.querySelectorAll("[data-page-break]").length,
    rtl: doc.querySelectorAll("[dir=rtl]").length,
    textLength: doc.body.textContent?.length,
    fontFamilies: [
      ...new Set([...doc.querySelectorAll<HTMLElement>("span")].map((e) => e.style.fontFamily)),
    ],
  };
  writeFileSync("/tmp/missal-docx-import-evidence.json", JSON.stringify(evidence, null, 2));
  writeFileSync("/tmp/missal-docx-import.html", result.html);
  expect(evidence.tables).toBe(21);
  expect(evidence.breaks).toBe(14);
}, 30000);

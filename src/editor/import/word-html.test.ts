/** @vitest-environment jsdom */
import { expect, it } from "vite-plus/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { normalizeWordHtml, readWordPageLayout, windowsLineRatio } from "#/editor/import/word-html";

// Trimmed from Word's clipboard HTML for an Urdu document that uses "Heading 1" for body text.
const WORD_CLIPBOARD = `<html xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w="urn:schemas-microsoft-com:office:word">
<head><meta name=Generator content="Microsoft Word 15">
<style><!--
p.MsoNormal {margin-top:0in;margin-bottom:8.0pt;line-height:107%;font-size:11.0pt;font-family:"Calibri",sans-serif;mso-bidi-font-size:14.0pt;mso-bidi-font-family:"Jameel Noori Nastaleeq";}
h1 {margin:0in;mso-margin-top-alt:auto;line-height:normal;font-size:24.0pt;font-family:"Times New Roman",serif;font-weight:bold;mso-bidi-font-size:16.0pt;mso-bidi-font-weight:normal;}
@page WordSection1 {size:595.35pt 841.95pt;margin:13.5pt 54.0pt 22.5pt 49.5pt;}
--></style></head>
<body><!--StartFragment-->
<h1 dir=RTL><span lang=AR-SA>درخواست برائے ریمانڈ</span></h1>
<p class=MsoNormal dir=RTL><span lang=AR-SA>جناب عالی</span><span dir=LTR style='font-size:12.0pt'> ASI</span></p>
<!--EndFragment--></body></html>`;

const normalize = (ratio?: number) =>
  new DOMParser().parseFromString(
    normalizeWordHtml(WORD_CLIPBOARD, { lineHeightRatio: () => ratio }),
    "text/html",
  ).body;

it("inlines Word's heading style and uses complex-script size and weight for Urdu runs", () => {
  const body = normalize();
  const heading = body.querySelector("h1");
  expect(heading?.style.fontSize).toBe("24pt");
  const urdu = [...(heading?.querySelectorAll("span") ?? [])].find(
    (span) => span.children.length === 0 && span.textContent?.includes("درخواست"),
  );
  expect(urdu?.style.fontSize).toBe("16pt");
  expect(urdu?.style.fontWeight).toBe("normal");
  expect(heading?.style.marginTop).toBe("14pt");
});

it("keeps Latin runs at their own size", () => {
  const latin = [...normalize().querySelectorAll("span")].find(
    (span) => span.textContent === " ASI",
  );
  expect(latin?.style.fontSize).toBe("12pt");
});

it("scales Word's multiple and single line spacing by the font's single-line height", () => {
  const body = normalize(1.694);
  expect(body.querySelector("p")?.style.lineHeight).toBe("1.813");
  expect(body.querySelector("h1")?.style.lineHeight).toBe("1.694");
});

it("reads the section's paper size and margins", () => {
  const layout = readWordPageLayout(WORD_CLIPBOARD);
  expect(layout?.widthMm).toBeCloseTo(210.03, 1);
  expect(layout?.heightMm).toBeCloseTo(297.02, 1);
  expect(layout?.marginTopMm).toBeCloseTo(4.76, 1);
  expect(layout?.marginRightMm).toBeCloseTo(19.05, 1);
  expect(layout?.marginBottomMm).toBeCloseTo(7.94, 1);
  expect(layout?.marginLeftMm).toBeCloseTo(17.46, 1);
  expect(readWordPageLayout("<p>plain</p>")).toBeUndefined();
});

it("leaves non-Word HTML untouched", () => {
  expect(normalizeWordHtml("<p style='line-height:107%'>x</p>")).toBe(
    "<p style='line-height:107%'>x</p>",
  );
});

it("reads Word's single-line height from the bundled Urdu font's Windows metrics", () => {
  const font = readFileSync(resolve("public/Jameel Noori Nastaleeq.ttf"));
  const ratio = windowsLineRatio(font.buffer.slice(font.byteOffset, font.byteOffset + font.length));
  expect(ratio).toBeCloseTo(1.694, 3);
});

/** @vitest-environment jsdom */
import { expect, it } from "vite-plus/test";
import { createEditor } from "lexical";
import { captureEditorEnvelope } from "#/editor/envelope";
import { envelopeToHtml } from "#/editor/html-export";
import { EDITOR_HTML_CONFIG } from "#/editor/html-config";
import { insertSanitizedHtml } from "#/editor/import/convert";
import { EDITOR_NODES, EDITOR_THEME } from "#/editor/nodes/registry";

it("exports saved imported cell borders and typography without inventing a cell width", () => {
  const editor = createEditor({
    namespace: "print-roundtrip-test",
    html: EDITOR_HTML_CONFIG,
    nodes: [...EDITOR_NODES],
    theme: EDITOR_THEME,
    onError: (error) => {
      throw error;
    },
  });
  insertSanitizedHtml(
    editor,
    '<table style="width: 160mm; table-layout: fixed"><tbody><tr><td style="border: 0; padding: 0; vertical-align: middle"><p style="font-size: 16pt; line-height: 1.5; margin: 0; text-align: center"><span style="font-family: Arial; font-size: 14pt">گواہ</span></p></td></tr></tbody></table>',
  );
  const saved = captureEditorEnvelope(editor);
  const html = envelopeToHtml(saved);
  const document = new DOMParser().parseFromString(html, "text/html");
  const cell = document.querySelector("td");
  const paragraph = cell?.querySelector("p");
  expect(cell?.style.width).toBe("");
  expect(cell?.style.borderWidth).toBe("0px");
  expect(cell?.style.padding).toBe("0px");
  expect(cell?.style.verticalAlign).toBe("middle");
  expect(paragraph?.style.fontSize).toBe("16pt");
  expect(paragraph?.style.lineHeight).toBe("1.5");
  expect(paragraph?.style.margin).toBe("0px");
  expect(paragraph?.style.textAlign).toBe("center");
  expect(paragraph?.querySelector("span")?.style.fontSize).toBe("14pt");
  expect(document.querySelector("table")?.style.width).toBe("160mm");
  expect(document.body.textContent).toBe("گواہ");
});

/** @vitest-environment jsdom */
import { expect, it } from "vitest";
import { createEditor } from "lexical";
import { $generateHtmlFromNodes } from "@lexical/html";
import { EDITOR_HTML_CONFIG, registerImportedStyleRendering } from "#/editor/html-config";
import { EDITOR_NODES } from "#/editor/nodes/registry";
import { insertSanitizedHtml } from "#/editor/import/convert";
import { registerCompletedTokenConversion } from "#/editor/recognition";
import { createDefaultPlaceholders, indexPlaceholders } from "#/lib/placeholder";
import { DEFAULT_FIELD_MARKERS } from "#/lib/settings";

it("retains Word paragraph, cell, run and field styling after JSON save/reopen and HTML export", () => {
  const editor = createEditor({
    namespace: "styled-import",
    nodes: [...EDITOR_NODES],
    html: EDITOR_HTML_CONFIG,
    onError: (error) => {
      throw error;
    },
  });
  const root = document.createElement("div");
  document.body.append(root);
  editor.setRootElement(root);
  const stopStyles = registerImportedStyleRendering(editor);
  const catalog = indexPlaceholders(createDefaultPlaceholders());
  const stopTokens = registerCompletedTokenConversion(
    editor,
    () => catalog,
    () => DEFAULT_FIELD_MARKERS,
  );
  try {
    insertSanitizedHtml(
      editor,
      '<p dir="rtl" style="margin-bottom:3pt;line-height:1.2;text-align:center"><span style="font-family:serif;font-size:16pt;font-weight:bold">مقدمہ @ایف آئی آر نمبر@</span></p><table dir="rtl" style="width:450pt;border-collapse:collapse"><tr><td style="width:100pt;border:0;padding:4pt"><p>اول</p></td><td style="width:350pt;border:1pt solid black"><p>دوم</p></td></tr></table><p><img src="data:image/png;base64,aGVsbG8=" width="40" height="20"></p>',
    );
    const saved = editor.getEditorState().toJSON();
    editor.setEditorState(editor.parseEditorState(saved));
    const html = editor.getEditorState().read(() => $generateHtmlFromNodes(editor));
    const result = new DOMParser().parseFromString(html, "text/html");
    expect(result.querySelector("p")?.style.marginBottom).toBe("3pt");
    expect(result.querySelector("p")?.style.lineHeight).toBe("1.2");
    expect(result.querySelector("p")?.style.textAlign).toBe("center");
    expect(result.querySelector("[data-field]")?.getAttribute("style")).toContain(
      "font-size: 16pt",
    );
    expect(result.querySelector("table")?.style.width).toBe("450pt");
    expect(result.querySelector("td")?.style.width).toBe("100pt");
    expect(result.querySelector("td")?.style.borderWidth).toBe("0px");
    expect(result.querySelector("img")?.getAttribute("width")).toBe("40");
    expect(result.querySelector("img")?.src).toBe("data:image/png;base64,aGVsbG8=");
  } finally {
    stopTokens();
    stopStyles();
    editor.setRootElement(null);
    root.remove();
  }
});

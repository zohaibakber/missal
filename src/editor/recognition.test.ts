/** @vitest-environment jsdom */
import { expect, it } from "vitest";
import { $getRoot, createEditor } from "lexical";
import { insertSanitizedHtml } from "#/editor/import/convert";
import { EDITOR_NODES } from "#/editor/nodes/registry";
import { registerCompletedTokenConversion } from "#/editor/recognition";
import { createDefaultPlaceholders, indexPlaceholders } from "#/lib/placeholder";

it("converts all pasted tokens across paragraphs and tables without dropping surrounding text", () => {
  const editor = createEditor({
    namespace: "pasted-placeholders",
    nodes: [...EDITOR_NODES],
    onError: (error) => {
      throw error;
    },
  });
  const root = document.createElement("div");
  document.body.append(root);
  editor.setRootElement(root);
  const catalog = indexPlaceholders(createDefaultPlaceholders());
  const unregister = registerCompletedTokenConversion(editor, () => catalog);

  try {
    insertSanitizedHtml(
      editor,
      '<p dir="rtl">مقدمہ @fir_no@</p><p><strong>تاریخ @date@ جرم @offence@ آخر</strong></p>' +
        "<table><tr><td><p>@1@ @police_station@ @unknown_field@</p></td></tr></table>",
    );
    const state = editor.getEditorState().toJSON();
    const serialized = JSON.stringify(state);
    expect(serialized.match(/"type":"field"/g)).toHaveLength(6);
    expect(serialized.match(/"_tag":"CatalogField"/g)).toHaveLength(5);
    expect(serialized).toContain('"_tag":"UnresolvedToken","text":"unknown_field"');
    expect(serialized).not.toContain("@fir_no@");
    expect(root.querySelectorAll("strong").length).toBeGreaterThan(0);
    expect(root.querySelector("table")).not.toBeNull();
    expect(editor.getEditorState().read(() => $getRoot().getTextContent())).toContain(" آخر");

    editor.setEditorState(editor.parseEditorState(state));
    expect(editor.getEditorState().toJSON()).toEqual(state);
  } finally {
    unregister();
    editor.setRootElement(null);
    root.remove();
  }
});

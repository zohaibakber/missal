/** @vitest-environment jsdom */
import { expect, it } from "vitest";
import { $getRoot, createEditor } from "lexical";
import { insertSanitizedHtml } from "#/editor/import/convert";
import { EDITOR_NODES } from "#/editor/nodes/registry";
import { findPlaceholderToken, registerCompletedTokenConversion } from "#/editor/recognition";
import { createDefaultPlaceholders, indexPlaceholders } from "#/lib/placeholder";
import { DEFAULT_FIELD_MARKERS, FieldMarkers } from "#/lib/settings";

it("converts all pasted names across paragraphs and tables without dropping surrounding text", () => {
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
  const unregister = registerCompletedTokenConversion(
    editor,
    () => catalog,
    () => DEFAULT_FIELD_MARKERS,
  );

  try {
    insertSanitizedHtml(
      editor,
      '<p dir="rtl">مقدمہ @ایف آئی آر نمبر@</p><p><strong>تاریخ @تاریخ ایف آئی آر@ جرم @جرم@ آخر</strong></p>' +
        "<table><tr><td><p>ای میل a@b.pk @تھانہ نام@ @نامعلوم@</p></td></tr></table>",
    );
    const state = editor.getEditorState().toJSON();
    const serialized = JSON.stringify(state);
    expect(serialized.match(/"type":"field"/g)).toHaveLength(4);
    expect(serialized.match(/"_tag":"CatalogField"/g)).toHaveLength(4);
    expect(serialized).toContain("@نامعلوم@");
    expect(serialized).toContain("a@b.pk");
    expect(serialized).not.toContain("@جرم@");
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

it("finds names between custom markers", () => {
  const catalog = indexPlaceholders(createDefaultPlaceholders());
  const markers = new FieldMarkers({ open: "{{", close: "}}" });
  const text = "مقدمہ {{جرم}} اور @ضلع نام@";
  expect(findPlaceholderToken(text, markers, catalog)).toEqual({
    start: 6,
    end: 13,
    reference: { _tag: "CatalogField", id: 5 },
  });
  expect(findPlaceholderToken("@ضلع نام@", markers, catalog)).toBeUndefined();
  expect(findPlaceholderToken("x@y @جرم@", DEFAULT_FIELD_MARKERS, catalog)).toMatchObject({
    start: 4,
  });
});

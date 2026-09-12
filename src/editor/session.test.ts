/** @vitest-environment jsdom */
import { expect, it, vi } from "vitest";
import { $getRoot, $createTextNode, createEditor, HISTORY_PUSH_TAG, UNDO_COMMAND } from "lexical";
import { attachEditorSession, type EditorUiState } from "#/editor/session";
import { EDITOR_NODES, EDITOR_THEME } from "#/editor/nodes/registry";
import { emptyDocumentEnvelope } from "#/lib/document-format";
import { catalogFieldPresentation } from "#/lib/field";
import { indexPlaceholders } from "#/lib/placeholder";

it("marks an undo after saving as unsaved so the reverted document can be saved", async () => {
  const editor = createEditor({
    namespace: "undo-save",
    nodes: [...EDITOR_NODES],
    theme: EDITOR_THEME,
    onError: (error) => {
      throw error;
    },
  });
  const root = document.createElement("div");
  document.body.append(root);
  editor.setRootElement(root);
  const index = indexPlaceholders([]);
  let ui: EditorUiState | undefined;
  const session = attachEditorSession(editor, {
    getPlaceholderIndex: () => index,
    presentation: catalogFieldPresentation(index, "values"),
    onUiChange: (value) => {
      ui = value;
    },
  });
  try {
    session.loadEnvelope(emptyDocumentEnvelope());
    editor.update(
      () => {
        $getRoot()
          .selectEnd()
          .insertNodes([$createTextNode("First draft")]);
      },
      { discrete: true, tag: HISTORY_PUSH_TAG },
    );
    editor.update(
      () => {
        $getRoot()
          .selectEnd()
          .insertNodes([$createTextNode(" revised")]);
      },
      { discrete: true, tag: HISTORY_PUSH_TAG },
    );
    session.markSaved(session.captureEnvelope().contentRevision);
    expect(ui?.dirty).toBe(false);
    editor.dispatchCommand(UNDO_COMMAND, undefined);
    await vi.waitFor(() => expect(ui?.dirty).toBe(true));
    expect(editor.getEditorState().read(() => $getRoot().getTextContent())).toBe("First draft");
  } finally {
    session.dispose();
    editor.setRootElement(null);
    root.remove();
  }
});

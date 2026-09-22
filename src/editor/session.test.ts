/** @vitest-environment jsdom */
import { expect, it, vi } from "vitest";
import { $getRoot, $createTextNode, createEditor, HISTORY_PUSH_TAG, UNDO_COMMAND } from "lexical";
import { attachEditorSession, type EditorUiState } from "#/editor/session";
import { EDITOR_NODES, EDITOR_THEME } from "#/editor/nodes/registry";
import { DocumentEnvelope, PageLayout, emptyDocumentEnvelope } from "#/lib/document-format";
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

it("preserves imported page geometry when capturing and reloading a document", () => {
  const editor = createEditor({
    namespace: "page-layout-session",
    nodes: [...EDITOR_NODES],
    onError: (error) => {
      throw error;
    },
  });
  const index = indexPlaceholders([]);
  const session = attachEditorSession(editor, {
    getPlaceholderIndex: () => index,
    presentation: catalogFieldPresentation(index),
  });
  const pageLayout = new PageLayout({
    widthMm: 215.9,
    heightMm: 355.6,
    marginTopMm: 12.7,
    marginRightMm: 15,
    marginBottomMm: 19,
    marginLeftMm: 15,
  });
  try {
    session.loadEnvelope(
      new DocumentEnvelope({
        format: "missal-lexical",
        version: 1,
        state: emptyDocumentEnvelope().state,
        pageLayout,
      }),
    );
    expect(session.captureEnvelope().envelope.pageLayout).toEqual(pageLayout);
    session.loadEnvelope(emptyDocumentEnvelope());
    expect(session.captureEnvelope().envelope.pageLayout).toBeUndefined();
  } finally {
    session.dispose();
  }
});

it("keeps the existing document editable and saveable after a failed DOCX import", async () => {
  const editor = createEditor({
    namespace: "failed-import-session",
    nodes: [...EDITOR_NODES],
    onError: (error) => {
      throw error;
    },
  });
  const index = indexPlaceholders([]);
  const session = attachEditorSession(editor, {
    getPlaceholderIndex: () => index,
    presentation: catalogFieldPresentation(index),
  });
  try {
    session.loadEnvelope(emptyDocumentEnvelope());
    editor.update(
      () => {
        $getRoot()
          .selectEnd()
          .insertNodes([$createTextNode("Keep this draft")]);
      },
      { discrete: true },
    );
    const before = session.captureEnvelope().envelope;
    const importing = session.importDocx(new File(["invalid archive"], "broken.docx"));
    expect(editor.isEditable()).toBe(false);
    expect(session.tryBeginSave()).toBe(false);
    await expect(importing).rejects.toThrow();
    expect(session.captureEnvelope().envelope).toEqual(before);
    expect(editor.isEditable()).toBe(true);
    expect(session.tryBeginSave()).toBe(true);
    session.endSave();
  } finally {
    session.dispose();
  }
});

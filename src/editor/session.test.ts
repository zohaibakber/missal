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

it("holds the save lock around runSave and only marks a successful save", async () => {
  const editor = createEditor({
    namespace: "run-save",
    nodes: [...EDITOR_NODES],
    onError: (error) => {
      throw error;
    },
  });
  const index = indexPlaceholders([]);
  let ui: EditorUiState | undefined;
  const session = attachEditorSession(editor, {
    getPlaceholderIndex: () => index,
    presentation: catalogFieldPresentation(index),
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
          .insertNodes([$createTextNode("Draft")]);
      },
      { discrete: true },
    );
    expect(ui?.dirty).toBe(true);

    const failed = await session.runSave(async () => {
      expect(ui?.savePending).toBe(true);
      expect(await session.runSave(async () => ({ saved: true, value: "nested" }))).toBe(undefined);
      return { saved: false, value: "failed" };
    });
    expect(failed).toBe("failed");
    expect(ui).toMatchObject({ dirty: true, savePending: false });

    const saved = await session.runSave(async (captured) => {
      expect(captured.envelope.state).toBeTruthy();
      return { saved: true, value: "saved" };
    });
    expect(saved).toBe("saved");
    expect(ui).toMatchObject({ dirty: false, savePending: false });
  } finally {
    session.dispose();
  }
});

function typeInto(editor: ReturnType<typeof createEditor>, text: string) {
  editor.update(
    () => {
      $getRoot()
        .selectEnd()
        .insertNodes([$createTextNode(text)]);
    },
    { discrete: true, tag: HISTORY_PUSH_TAG },
  );
}

it("publishes the dirty transition once across repeated typing", async () => {
  const editor = createEditor({
    namespace: "dirty-publish",
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
  const published: EditorUiState[] = [];
  const session = attachEditorSession(editor, {
    getPlaceholderIndex: () => index,
    presentation: catalogFieldPresentation(index, "values"),
    onUiChange: (value) => {
      published.push(value);
    },
  });
  try {
    session.loadEnvelope(emptyDocumentEnvelope());
    typeInto(editor, "ا");
    await vi.waitFor(() => expect(published.at(-1)).toMatchObject({ canUndo: true, dirty: true }));
    const settled = published.length;

    typeInto(editor, "ب");
    typeInto(editor, "پ");
    typeInto(editor, "ت");
    expect(published.length).toBe(settled);
    expect(published.at(-1)).toMatchObject({ canRedo: false, canUndo: true, dirty: true });
    expect(session.captureEnvelope().contentRevision).toBeGreaterThan(1);
  } finally {
    session.dispose();
    editor.setRootElement(null);
    root.remove();
  }
});

it("leaves the document dirty when typing continues during a save", async () => {
  const editor = createEditor({
    namespace: "save-while-typing",
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
    typeInto(editor, "پہلا");
    await vi.waitFor(() => expect(ui?.dirty).toBe(true));

    const saved = await session.runSave(async (captured) => {
      typeInto(editor, " مسودہ");
      expect(session.captureEnvelope().contentRevision).toBeGreaterThan(captured.contentRevision);
      return { saved: true, value: "saved" };
    });

    expect(saved).toBe("saved");
    expect(ui).toMatchObject({ dirty: true, savePending: false });
    expect(editor.getEditorState().read(() => $getRoot().getTextContent())).toBe("پہلا مسودہ");
  } finally {
    session.dispose();
    editor.setRootElement(null);
    root.remove();
  }
});

it("publishes toolbar state when undo or saving changes", async () => {
  const editor = createEditor({
    namespace: "toolbar-publish",
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
  const published: EditorUiState[] = [];
  const session = attachEditorSession(editor, {
    getPlaceholderIndex: () => index,
    presentation: catalogFieldPresentation(index, "values"),
    onUiChange: (value) => {
      published.push(value);
    },
  });
  try {
    session.loadEnvelope(emptyDocumentEnvelope());
    typeInto(editor, "پہلا");
    typeInto(editor, " دوسرا");
    await vi.waitFor(() => expect(published.at(-1)).toMatchObject({ canUndo: true, dirty: true }));
    const typed = published.length;

    editor.dispatchCommand(UNDO_COMMAND, undefined);
    await vi.waitFor(() => expect(published.length).toBeGreaterThan(typed));
    expect(published.at(-1)).toMatchObject({ canRedo: true, canUndo: true });

    const beforeSave = published.length;
    expect(session.tryBeginSave()).toBe(true);
    expect(published.at(-1)?.savePending).toBe(true);
    expect(published.length).toBe(beforeSave + 1);
    session.endSave();
    expect(published.at(-1)).toMatchObject({ dirty: true, savePending: false });
    expect(published.length).toBe(beforeSave + 2);
  } finally {
    session.dispose();
    editor.setRootElement(null);
    root.remove();
  }
});

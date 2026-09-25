import { createEmptyHistoryState, registerHistory } from "@lexical/history";
import {
  $getRoot,
  $insertNodes,
  CLEAR_HISTORY_COMMAND,
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  COMMAND_PRIORITY_LOW,
  type LexicalEditor,
} from "lexical";
import type { DocumentEnvelope, PageLayout } from "#/lib/document-format";
import type { FieldPresentationContext } from "#/lib/field";
import type { PlaceholderIndex } from "#/lib/placeholder";
import { DEFAULT_FIELD_MARKERS, type FieldMarkers } from "#/lib/settings";
import { captureEditorEnvelope, loadEnvelopeIntoEditor } from "#/editor/envelope";
import { $generateNodesFromDOM } from "@lexical/html";
import { registerImportedStyleRendering } from "#/editor/html-config";
import { registerClipboardImport } from "#/editor/import/clipboard";
import { FieldPresentationController } from "#/editor/presentation";
import {
  registerCompletedTokenConversion,
  registerFieldRecognition,
  registerFieldReveal,
} from "#/editor/recognition";

type EditorPhase =
  | { readonly _tag: "Loading" }
  | { readonly _tag: "Ready" }
  | { readonly _tag: "Importing"; readonly progress: number }
  | { readonly _tag: "Failed"; readonly message: string }
  | { readonly _tag: "Disposed" };

export type EditorUiState = {
  phase: EditorPhase;
  pageLayout?: PageLayout;
  dirty: boolean;
  empty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  savePending: boolean;
};

type CapturedEnvelope = {
  readonly envelope: DocumentEnvelope;
  readonly contentRevision: number;
};

export type EditorSessionHandle = {
  captureEnvelope(): CapturedEnvelope;
  importDocx(file: File): Promise<string[]>;
  loadEnvelope(envelope: DocumentEnvelope): void;
  setPresentation(context: FieldPresentationContext): void;
  markSaved(contentRevision: number): void;
  tryBeginSave(): boolean;
  endSave(): void;
  /**
   * Runs `save` on a fresh capture while holding the save lock, and marks the capture saved when
   * `save` reports success. Resolves `undefined` when another save or an import is running.
   */
  runSave<A>(
    save: (captured: CapturedEnvelope) => Promise<{ readonly saved: boolean; readonly value: A }>,
  ): Promise<A | undefined>;
  dispose(): void;
};

const HISTORY_MAX_DEPTH = 100;

function editorUiDefaults(): EditorUiState {
  return {
    canRedo: false,
    canUndo: false,
    dirty: false,
    empty: true,
    phase: { _tag: "Loading" },
    savePending: false,
  };
}

export function attachEditorSession(
  editor: LexicalEditor,
  options: {
    getPlaceholderIndex: () => PlaceholderIndex;
    getFieldMarkers?: () => FieldMarkers;
    presentation: FieldPresentationContext;
    onUiChange?: (ui: EditorUiState) => void;
  },
): EditorSessionHandle {
  const ui: EditorUiState = editorUiDefaults();
  let contentRevision = 0;
  let disposed = false;
  const history = createEmptyHistoryState();
  const presentation = new FieldPresentationController(editor, options.presentation);
  const getFieldMarkers = options.getFieldMarkers ?? (() => DEFAULT_FIELD_MARKERS);
  const unregisters = [
    registerImportedStyleRendering(editor),
    registerHistory(editor, history, 300, Date.now, undefined, HISTORY_MAX_DEPTH),
    presentation.attach(),
    registerFieldRecognition(editor, options.getPlaceholderIndex, getFieldMarkers),
    registerCompletedTokenConversion(editor, options.getPlaceholderIndex, getFieldMarkers),
    registerFieldReveal(editor, options.getPlaceholderIndex, getFieldMarkers),
    registerClipboardImport(editor, {
      onPageLayout: (pageLayout) => {
        // The first Word paste defines the page; later pastes don't reflow an existing layout.
        if (ui.pageLayout) return;
        ui.pageLayout = pageLayout;
        notify();
      },
    }),
    editor.registerCommand(
      CAN_UNDO_COMMAND,
      (payload) => {
        ui.canUndo = payload;
        notify();
        return false;
      },
      COMMAND_PRIORITY_LOW,
    ),
    editor.registerCommand(
      CAN_REDO_COMMAND,
      (payload) => {
        ui.canRedo = payload;
        notify();
        return false;
      },
      COMMAND_PRIORITY_LOW,
    ),
    editor.registerUpdateListener(({ dirtyLeaves, dirtyElements }) => {
      if (ui.phase._tag === "Loading" || ui.phase._tag === "Disposed") {
        return;
      }

      if (dirtyLeaves.size > 0 || dirtyElements.size > 0) {
        contentRevision += 1;
        ui.dirty = true;
        notify();
      }
    }),
  ];

  function notify() {
    if (!disposed) {
      options.onUiChange?.({ ...ui });
    }
  }

  function setPhase(phase: EditorPhase) {
    ui.phase = phase;
    notify();
  }

  function tryBeginSave() {
    if (ui.savePending || ui.phase._tag !== "Ready") {
      return false;
    }

    ui.savePending = true;
    notify();
    return true;
  }

  function endSave() {
    ui.savePending = false;
    notify();
  }

  function markSaved(savedContentRevision: number) {
    if (contentRevision === savedContentRevision) {
      ui.dirty = false;
      notify();
    }
  }

  function captureEnvelope(): CapturedEnvelope {
    return {
      contentRevision,
      envelope: captureEditorEnvelope(editor, ui.pageLayout),
    };
  }

  return {
    captureEnvelope,
    async importDocx(file) {
      if (disposed || ui.phase._tag !== "Ready" || ui.savePending) {
        throw new Error("Wait for the current operation before importing a document.");
      }
      const wasEditable = editor.isEditable();
      editor.setEditable(false);
      setPhase({ _tag: "Importing", progress: 0 });
      try {
        const [{ importDocx }, { prepareClipboardDom }] = await Promise.all([
          import("#/editor/import/docx"),
          import("#/editor/import/convert"),
        ]);
        const imported = await importDocx(file);
        if (disposed) {
          throw new Error("The template was closed before the import finished.");
        }
        const { dom } = prepareClipboardDom(imported.html);
        editor.update(
          () => {
            const nodes = $generateNodesFromDOM(editor, dom);
            const root = $getRoot();
            root.clear();
            root.selectStart();
            $insertNodes(nodes);
          },
          { discrete: true },
        );
        ui.pageLayout = imported.pageLayout;
        ui.dirty = true;
        ui.empty = false;
        editor.dispatchCommand(CLEAR_HISTORY_COMMAND, undefined);
        return imported.notices;
      } finally {
        if (!disposed) {
          editor.setEditable(wasEditable);
          setPhase({ _tag: "Ready" });
        }
      }
    },
    loadEnvelope(envelope) {
      ui.phase = { _tag: "Loading" };
      ui.pageLayout = envelope.pageLayout;
      loadEnvelopeIntoEditor(editor, envelope);
      contentRevision = 0;
      ui.dirty = false;
      ui.empty = false;
      setPhase({ _tag: "Ready" });
    },
    setPresentation(context) {
      presentation.setContext(context);
    },
    markSaved,
    tryBeginSave,
    endSave,
    async runSave(save) {
      if (!tryBeginSave()) {
        return undefined;
      }

      try {
        const captured = captureEnvelope();
        const result = await save(captured);
        if (result.saved) {
          markSaved(captured.contentRevision);
        }
        return result.value;
      } finally {
        endSave();
      }
    },
    dispose() {
      disposed = true;
      ui.phase = { _tag: "Disposed" };
      for (const unregister of unregisters) {
        unregister();
      }
    },
  };
}

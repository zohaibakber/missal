import { createEmptyHistoryState, registerHistory } from "@lexical/history";
import {
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  COMMAND_PRIORITY_LOW,
  type LexicalEditor,
} from "lexical";
import type { DocumentEnvelope } from "#/lib/document-format";
import type { FieldPresentationContext } from "#/lib/field";
import type { PlaceholderIndex } from "#/lib/placeholder";
import { captureEditorEnvelope, loadEnvelopeIntoEditor } from "#/editor/envelope";
import { registerClipboardImport } from "#/editor/import/convert";
import { FieldPresentationController } from "#/editor/presentation";
import { registerCompletedTokenConversion, registerFieldRecognition } from "#/editor/recognition";

export type EditorPhase =
  | { readonly _tag: "Loading" }
  | { readonly _tag: "Ready" }
  | { readonly _tag: "Importing"; readonly progress: number }
  | { readonly _tag: "Failed"; readonly message: string }
  | { readonly _tag: "Disposed" };

export type EditorUiState = {
  phase: EditorPhase;
  dirty: boolean;
  empty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  savePending: boolean;
};

export type CapturedEnvelope = {
  readonly envelope: DocumentEnvelope;
  readonly contentRevision: number;
};

export type EditorSessionHandle = {
  captureEnvelope(): CapturedEnvelope;
  loadEnvelope(envelope: DocumentEnvelope): void;
  setPresentation(context: FieldPresentationContext): void;
  markSaved(contentRevision: number): void;
  tryBeginSave(): boolean;
  endSave(): void;
  dispose(): void;
};

export const HISTORY_MAX_DEPTH = 100;

export function editorUiDefaults(): EditorUiState {
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
    presentation: FieldPresentationContext;
    onUiChange?: (ui: EditorUiState) => void;
  },
): EditorSessionHandle {
  const ui: EditorUiState = editorUiDefaults();
  let contentRevision = 0;
  let disposed = false;
  const history = createEmptyHistoryState();
  const presentation = new FieldPresentationController(editor, options.presentation);
  const unregisters = [
    registerHistory(editor, history, 300, Date.now, undefined, HISTORY_MAX_DEPTH),
    presentation.attach(),
    registerFieldRecognition(editor, options.getPlaceholderIndex),
    registerCompletedTokenConversion(editor, options.getPlaceholderIndex),
    registerClipboardImport(editor),
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

  return {
    captureEnvelope() {
      return {
        contentRevision,
        envelope: captureEditorEnvelope(editor),
      };
    },
    loadEnvelope(envelope) {
      ui.phase = { _tag: "Loading" };
      loadEnvelopeIntoEditor(editor, envelope);
      contentRevision = 0;
      ui.dirty = false;
      ui.empty = false;
      setPhase({ _tag: "Ready" });
    },
    setPresentation(context) {
      presentation.setContext(context);
    },
    markSaved(savedContentRevision) {
      if (contentRevision === savedContentRevision) {
        ui.dirty = false;
        notify();
      }
    },
    tryBeginSave() {
      if (ui.savePending || ui.phase._tag !== "Ready") {
        return false;
      }

      ui.savePending = true;
      notify();
      return true;
    },
    endSave() {
      ui.savePending = false;
      notify();
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

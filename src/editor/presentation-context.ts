import type { LexicalEditor } from "lexical";
import type { FieldPresentationContext } from "#/lib/field";

const presentationByEditor = new WeakMap<LexicalEditor, FieldPresentationContext>();

export function getEditorPresentation(editor: LexicalEditor) {
  return presentationByEditor.get(editor);
}

export function setEditorPresentation(editor: LexicalEditor, context: FieldPresentationContext) {
  presentationByEditor.set(editor, context);
}

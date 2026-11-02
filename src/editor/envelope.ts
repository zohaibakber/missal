import type { LexicalEditor, SerializedEditorState } from "lexical";
import {
  DOCUMENT_FORMAT,
  DOCUMENT_FORMAT_VERSION,
  DocumentEnvelope,
  expandDocumentStyles,
  normalizeDocumentStyles,
  type PageLayout,
} from "#/lib/document-format";
import { retainedStyle } from "#/editor/imported-style";

export function loadEnvelopeIntoEditor(editor: LexicalEditor, envelope: DocumentEnvelope) {
  const state = expandDocumentStyles({
    state: envelope.state,
    styles: envelope.styles.map((style) => retainedStyle(style)),
  });
  editor.setEditorState(editor.parseEditorState(state as SerializedEditorState));
}

export function captureEditorEnvelope(editor: LexicalEditor, pageLayout?: PageLayout) {
  return new DocumentEnvelope({
    format: DOCUMENT_FORMAT,
    ...(pageLayout ? { pageLayout } : {}),
    ...normalizeDocumentStyles(editor.getEditorState().toJSON(true)),
    version: DOCUMENT_FORMAT_VERSION,
  });
}

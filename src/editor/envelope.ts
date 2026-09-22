import type { LexicalEditor, SerializedEditorState } from "lexical";
import {
  DOCUMENT_FORMAT,
  DOCUMENT_FORMAT_VERSION,
  DocumentEnvelope,
  type PageLayout,
} from "#/lib/document-format";

export function loadEnvelopeIntoEditor(editor: LexicalEditor, envelope: DocumentEnvelope) {
  editor.setEditorState(editor.parseEditorState(envelope.state as SerializedEditorState));
}

export function captureEditorEnvelope(editor: LexicalEditor, pageLayout?: PageLayout) {
  return new DocumentEnvelope({
    format: DOCUMENT_FORMAT,
    ...(pageLayout ? { pageLayout } : {}),
    state: editor.getEditorState().toJSON(),
    version: DOCUMENT_FORMAT_VERSION,
  });
}

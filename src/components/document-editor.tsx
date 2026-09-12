import { useEffect, useRef } from "react";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { ListPlugin } from "@lexical/react/LexicalListPlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { TablePlugin } from "@lexical/react/LexicalTablePlugin";
import { LinkPlugin } from "@lexical/react/LexicalLinkPlugin";
import { HorizontalRulePlugin } from "@lexical/react/LexicalHorizontalRulePlugin";
import { EDITOR_NODES, EDITOR_THEME } from "#/editor/nodes/registry";
import type { FieldPresentationContext } from "#/lib/field";
import {
  attachEditorSession,
  type EditorSessionHandle,
  type EditorUiState,
} from "#/editor/session";
import type { DocumentEnvelope } from "#/lib/document-format";
import type { PlaceholderIndex } from "#/lib/placeholder";
import { EditorToolbar } from "#/components/editor-toolbar";
import { cn } from "#/lib/utils";

export type DocumentEditorHandle = EditorSessionHandle;

type DocumentEditorProps = {
  "aria-label": string;
  className?: string;
  document: DocumentEnvelope;
  sessionKey: string | number;
  placeholderIndex: PlaceholderIndex;
  presentation: FieldPresentationContext;
  onDirtyChange?: (dirty: boolean) => void;
  onSavePendingChange?: (savePending: boolean) => void;
  onHistoryChange?: (canUndo: boolean, canRedo: boolean) => void;
  onSessionReady?: (session: EditorSessionHandle | null) => void;
  editable?: boolean;
};

function SessionPlugins({
  document: envelope,
  placeholderIndex,
  presentation,
  onDirtyChange,
  onSavePendingChange,
  onHistoryChange,
  onSessionReady,
}: Omit<DocumentEditorProps, "aria-label" | "className" | "editable" | "sessionKey">) {
  const [editor] = useLexicalComposerContext();
  const indexRef = useRef(placeholderIndex);
  indexRef.current = placeholderIndex;
  const presentationRef = useRef(presentation);
  presentationRef.current = presentation;
  const envelopeRef = useRef(envelope);
  const uiHandlersRef = useRef({ onDirtyChange, onHistoryChange, onSavePendingChange });
  uiHandlersRef.current = { onDirtyChange, onHistoryChange, onSavePendingChange };
  const onSessionReadyRef = useRef(onSessionReady);
  onSessionReadyRef.current = onSessionReady;
  const sessionRef = useRef<EditorSessionHandle | null>(null);

  useEffect(() => {
    const session = attachEditorSession(editor, {
      getPlaceholderIndex: () => indexRef.current,
      onUiChange: (ui: EditorUiState) => {
        const handlers = uiHandlersRef.current;
        handlers.onDirtyChange?.(ui.dirty);
        handlers.onHistoryChange?.(ui.canUndo, ui.canRedo);
        handlers.onSavePendingChange?.(ui.savePending);
      },
      presentation: presentationRef.current,
    });
    session.loadEnvelope(envelopeRef.current);
    sessionRef.current = session;
    onSessionReadyRef.current?.(session);

    return () => {
      session.dispose();
      sessionRef.current = null;
      onSessionReadyRef.current?.(null);
    };
  }, [editor]);

  useEffect(() => {
    sessionRef.current?.setPresentation(presentation);
  }, [presentation]);

  return null;
}

export function DocumentEditor({
  "aria-label": ariaLabel,
  className,
  document: envelope,
  editable = true,
  placeholderIndex,
  presentation,
  sessionKey,
  onDirtyChange,
  onSavePendingChange,
  onHistoryChange,
  onSessionReady,
}: DocumentEditorProps) {
  return (
    <LexicalComposer
      key={String(sessionKey)}
      initialConfig={{
        editable,
        namespace: "missal-document",
        nodes: [...EDITOR_NODES],
        onError: (error) => {
          console.error("Missal editor error", error);
        },
        theme: EDITOR_THEME,
      }}
    >
      <div className={cn("missal-editor relative flex h-full min-h-0 min-w-0 flex-col", className)}>
        {editable && <EditorToolbar />}
        <RichTextPlugin
          contentEditable={
            <ContentEditable
              aria-label={ariaLabel}
              lang="ur"
              dir="rtl"
              className="missal-editor-input min-h-0 w-full min-w-0 flex-1 overflow-y-auto bg-background px-6 py-8 outline-none md:px-12"
            />
          }
          ErrorBoundary={LexicalErrorBoundary}
        />
        <ListPlugin />
        <TablePlugin />
        <LinkPlugin />
        <HorizontalRulePlugin />
        <SessionPlugins
          document={envelope}
          onDirtyChange={onDirtyChange}
          onHistoryChange={onHistoryChange}
          onSavePendingChange={onSavePendingChange}
          onSessionReady={onSessionReady}
          placeholderIndex={placeholderIndex}
          presentation={presentation}
        />
      </div>
    </LexicalComposer>
  );
}

import { useEffect, useRef, useState } from "react";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { useLexicalEditable } from "@lexical/react/useLexicalEditable";
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
import { EDITOR_HTML_CONFIG } from "#/editor/html-config";
import type { DocumentEnvelope, PageLayout } from "#/lib/document-format";
import type { PlaceholderIndex } from "#/lib/placeholder";
import { EditorToolbar } from "#/components/editor-toolbar";
import { cn } from "#/lib/utils";

function EditableToolbar() {
  const editable = useLexicalEditable();
  return editable ? <EditorToolbar /> : null;
}

type DocumentEditorProps = {
  "aria-label": string;
  className?: string;
  document: DocumentEnvelope;
  sessionKey: string | number;
  placeholderIndex: PlaceholderIndex;
  presentation: FieldPresentationContext;
  onPageLayoutChange?: (layout: PageLayout | undefined) => void;
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
  onPageLayoutChange,
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
  const uiHandlersRef = useRef({
    onDirtyChange,
    onHistoryChange,
    onSavePendingChange,
    onPageLayoutChange,
  });
  uiHandlersRef.current = {
    onDirtyChange,
    onHistoryChange,
    onSavePendingChange,
    onPageLayoutChange,
  };
  const onSessionReadyRef = useRef(onSessionReady);
  onSessionReadyRef.current = onSessionReady;
  const sessionRef = useRef<EditorSessionHandle | null>(null);

  useEffect(() => {
    const session = attachEditorSession(editor, {
      getPlaceholderIndex: () => indexRef.current,
      onUiChange: (ui: EditorUiState) => {
        const handlers = uiHandlersRef.current;
        handlers.onDirtyChange?.(ui.dirty);
        handlers.onPageLayoutChange?.(ui.pageLayout);
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

const MM_TO_PX = 96 / 25.4;
const DEFAULT_PAGE_WIDTH_MM = 210;
// Horizontal breathing room kept around the page when it is scaled down to fit.
const CANVAS_GUTTER_PX = 48;

/**
 * Scales the page down (never up) so the whole width is visible, like "fit width" in Word.
 * Writes a CSS variable instead of state so resizing never re-renders the editor.
 */
function usePageFit(pageWidthMm: number) {
  const canvasRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const pageWidthPx = pageWidthMm * MM_TO_PX;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const zoom = Math.min(1, (entry.contentRect.width - CANVAS_GUTTER_PX) / pageWidthPx);
      canvas.style.setProperty("--page-zoom", `${Math.max(zoom, 0.25)}`);
    });
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [pageWidthMm]);

  return canvasRef;
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
  const [pageLayout, setPageLayout] = useState(envelope.pageLayout);
  const canvasRef = usePageFit(pageLayout?.widthMm ?? DEFAULT_PAGE_WIDTH_MM);
  return (
    <LexicalComposer
      key={String(sessionKey)}
      initialConfig={{
        editable,
        html: EDITOR_HTML_CONFIG,
        namespace: "missal-document",
        nodes: [...EDITOR_NODES],
        onError: (error) => {
          console.error("Missal editor error", error);
        },
        theme: EDITOR_THEME,
      }}
    >
      <div className={cn("missal-editor relative flex h-full min-h-0 min-w-0 flex-col", className)}>
        <EditableToolbar />
        <div ref={canvasRef} className="min-h-0 flex-1 overflow-auto bg-canvas">
          <RichTextPlugin
            contentEditable={
              <ContentEditable
                aria-label={ariaLabel}
                lang="ur"
                dir="rtl"
                className="missal-editor-input missal-page mx-auto outline-none select-text"
                style={
                  pageLayout
                    ? {
                        "--page-width": `${pageLayout.widthMm}mm`,
                        "--page-height": `${pageLayout.heightMm}mm`,
                        "--page-top": `${pageLayout.marginTopMm}mm`,
                        "--page-right": `${pageLayout.marginRightMm}mm`,
                        "--page-bottom": `${pageLayout.marginBottomMm}mm`,
                        "--page-left": `${pageLayout.marginLeftMm}mm`,
                      }
                    : undefined
                }
              />
            }
            ErrorBoundary={LexicalErrorBoundary}
          />
        </div>
        <ListPlugin />
        <TablePlugin />
        <LinkPlugin />
        <HorizontalRulePlugin />
        <SessionPlugins
          document={envelope}
          onPageLayoutChange={setPageLayout}
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

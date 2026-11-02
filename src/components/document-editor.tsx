import { useEffect, useEffectEvent, useRef, useState } from "react";
import { useAtomValue } from "@effect/atom-react";
import { HashMap } from "effect";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalExtensionComposer } from "@lexical/react/LexicalExtensionComposer";
import { useLexicalEditable } from "@lexical/react/useLexicalEditable";
import { DocumentEditorExtension } from "#/editor/extension";
import { FIR_FIELDS, type FieldPresentationContext } from "#/lib/field";
import {
  attachEditorSession,
  type EditorSessionHandle,
  type EditorUiState,
} from "#/editor/session";
import type { DocumentEnvelope, PageLayout } from "#/lib/document-format";
import type { FieldIndex } from "#/lib/placeholder";
import { EditorToolbar, type InsertableFieldGroup } from "#/components/editor-toolbar";
import { cn } from "#/lib/utils";
import { atoms } from "#/state/atoms";

function fieldGroups(fieldIndex: FieldIndex): InsertableFieldGroup[] {
  const placeholders = [...HashMap.values(fieldIndex.byId)];
  return [
    { heading: "FIR", labels: FIR_FIELDS.map((field) => field.label) },
    {
      heading: "Global",
      labels: placeholders
        .filter((placeholder) => placeholder.source._tag !== "Custom")
        .map((placeholder) => placeholder.label),
    },
    {
      heading: "Custom",
      labels: placeholders
        .filter((placeholder) => placeholder.source._tag === "Custom")
        .map((placeholder) => placeholder.label),
    },
  ];
}

function EditableToolbar({ fieldIndex }: { fieldIndex: FieldIndex }) {
  const editable = useLexicalEditable();
  return editable ? <EditorToolbar fieldGroups={fieldGroups(fieldIndex)} /> : null;
}

type DocumentEditorProps = {
  "aria-label": string;
  className?: string;
  document: DocumentEnvelope;
  sessionKey: string | number;
  fieldIndex: FieldIndex;
  presentation: FieldPresentationContext;
  onPageLayoutChange?: (layout: PageLayout | undefined) => void;
  onDirtyChange?: (dirty: boolean) => void;
  onSavePendingChange?: (savePending: boolean) => void;
  onSessionReady?: (session: EditorSessionHandle | null) => void;
};

function SessionPlugins({
  document: envelope,
  fieldIndex,
  presentation,
  onPageLayoutChange,
  onDirtyChange,
  onSavePendingChange,
  onSessionReady,
}: Omit<DocumentEditorProps, "aria-label" | "className" | "sessionKey">) {
  const [editor] = useLexicalComposerContext();
  const fieldMarkers = useAtomValue(atoms.fieldMarkersAtom);
  const [initialEnvelope] = useState(envelope);
  const [initialPresentation] = useState(presentation);
  const sessionRef = useRef<EditorSessionHandle | null>(null);

  const getFieldIndex = useEffectEvent(() => fieldIndex);
  const getFieldMarkers = useEffectEvent(() => fieldMarkers);
  const reportUi = useEffectEvent((ui: EditorUiState) => {
    onDirtyChange?.(ui.dirty);
    onPageLayoutChange?.(ui.pageLayout);
    onSavePendingChange?.(ui.savePending);
  });
  const announceSession = useEffectEvent((session: EditorSessionHandle | null) => {
    onSessionReady?.(session);
  });

  useEffect(() => {
    const session = attachEditorSession(editor, {
      getFieldIndex: () => getFieldIndex(),
      getFieldMarkers: () => getFieldMarkers(),
      onUiChange: (ui) => reportUi(ui),
      presentation: initialPresentation,
    });
    session.loadEnvelope(initialEnvelope);
    sessionRef.current = session;
    announceSession(session);

    return () => {
      session.dispose();
      sessionRef.current = null;
      announceSession(null);
    };
  }, [editor, initialEnvelope, initialPresentation]);

  useEffect(() => {
    sessionRef.current?.setPresentation(presentation);
  }, [presentation]);

  return null;
}

const MM_TO_PX = 96 / 25.4;
const DEFAULT_PAGE_WIDTH_MM = 210;
const CANVAS_GUTTER_PX = 48;

function usePageFit(pageWidthMm: number) {
  const canvasRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const pageWidthPx = pageWidthMm * MM_TO_PX;
    let frame = 0;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const zoom = `${Math.max(Math.min(1, (entry.contentRect.width - CANVAS_GUTTER_PX) / pageWidthPx), 0.25)}`;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        if (canvas.style.getPropertyValue("--page-zoom") !== zoom) {
          canvas.style.setProperty("--page-zoom", zoom);
        }
      });
    });
    observer.observe(canvas);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [pageWidthMm]);

  return canvasRef;
}

export function DocumentEditor({
  "aria-label": ariaLabel,
  className,
  document: envelope,
  fieldIndex,
  presentation,
  sessionKey,
  onDirtyChange,
  onSavePendingChange,
  onSessionReady,
}: DocumentEditorProps) {
  const [pageLayout, setPageLayout] = useState(envelope.pageLayout);
  const canvasRef = usePageFit(pageLayout?.widthMm ?? DEFAULT_PAGE_WIDTH_MM);
  return (
    <LexicalExtensionComposer
      key={String(sessionKey)}
      extension={DocumentEditorExtension}
      contentEditable={null}
    >
      <div className={cn("missal-editor relative flex h-full min-h-0 min-w-0 flex-col", className)}>
        <EditableToolbar fieldIndex={fieldIndex} />
        <div ref={canvasRef} className="min-h-0 flex-1 overflow-auto bg-canvas">
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
        </div>
        <SessionPlugins
          document={envelope}
          onPageLayoutChange={setPageLayout}
          onDirtyChange={onDirtyChange}
          onSavePendingChange={onSavePendingChange}
          onSessionReady={onSessionReady}
          fieldIndex={fieldIndex}
          presentation={presentation}
        />
      </div>
    </LexicalExtensionComposer>
  );
}

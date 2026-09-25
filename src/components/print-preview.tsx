import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { PDFDocumentLoadingTask, PDFDocumentProxy } from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import {
  ArrowLeft01Icon,
  ArrowRight01Icon,
  MinusSignIcon,
  PlusSignIcon,
  PrinterIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Hint } from "#/components/hint";
import { IconAction } from "#/components/icon-action";
import { Button } from "#/components/ui/button";
import { Input } from "#/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "#/components/ui/dialog";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "#/components/ui/empty";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import { Spinner } from "#/components/ui/spinner";
import { useShortcut } from "#/hooks/use-shortcut";
import type { PrintPacket } from "#/editor/html-export";

// PDF user space is in points (1/72 in); CSS pixels are 1/96 in.
const POINTS_TO_CSS_PX = 96 / 72;
// Keep in sync with the page list's gap-6 / px-12.
const PAGE_GAP_PX = 24;
const CANVAS_PADDING_PX = 48;
const ZOOM_STEPS = [50, 75, 100, 125, 150, 200] as const;

type Zoom = "fit" | (typeof ZOOM_STEPS)[number];

const ZOOM_ITEMS: readonly { value: Zoom; label: string }[] = [
  { value: "fit", label: "Fit width" },
  ...ZOOM_STEPS.map((step) => ({ value: step, label: `${step}%` })),
];
type PageSize = { readonly width: number; readonly height: number };

type PreviewState =
  | { readonly _tag: "Loading" }
  | { readonly _tag: "Ready"; readonly pdf: PDFDocumentProxy; readonly pages: readonly PageSize[] }
  | { readonly _tag: "Failed"; readonly message: string };

/** Renders the packet through the same Chromium print pipeline as the printer, then loads it. */
async function loadPrintPdf(html: string, onTask: (task: PDFDocumentLoadingTask) => void) {
  const api = window.electronPrint;
  if (!api) throw new Error("Print preview is only available in the Missal desktop app.");
  const [data, pdfjs] = await Promise.all([api.renderPdf(html), import("pdfjs-dist")]);
  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;
  const loadingTask = pdfjs.getDocument({ data });
  onTask(loadingTask);
  const pdf = await loadingTask.promise;
  const pages = await Promise.all(
    Array.from({ length: pdf.numPages }, async (_, index) => {
      const viewport = (await pdf.getPage(index + 1)).getViewport({ scale: 1 });
      return { width: viewport.width, height: viewport.height };
    }),
  );
  return { pdf, pages };
}

function usePrintPdf(packet: PrintPacket | null, attempt: number) {
  const [state, setState] = useState<PreviewState>({ _tag: "Loading" });

  useEffect(() => {
    if (!packet) return;
    let cancelled = false;
    let loadingTask: PDFDocumentLoadingTask | undefined;
    setState({ _tag: "Loading" });

    loadPrintPdf(packet.html, (task) => {
      if (cancelled) void task.destroy();
      else loadingTask = task;
    }).then(
      ({ pdf, pages }) => {
        if (!cancelled) setState({ _tag: "Ready", pdf, pages });
      },
      (error: unknown) => {
        if (!cancelled) {
          setState({
            _tag: "Failed",
            message: error instanceof Error ? error.message : "The pages could not be prepared.",
          });
        }
      },
    );

    return () => {
      cancelled = true;
      void loadingTask?.destroy();
    };
  }, [packet, attempt]);

  return state;
}

/** Draws one page only while it is near the viewport, so long packets stay light. */
function PreviewPage({
  pdf,
  pageNumber,
  size,
  scale,
  root,
}: {
  pdf: PDFDocumentProxy;
  pageNumber: number;
  size: PageSize;
  scale: number;
  root: HTMLElement | null;
}) {
  const frameRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [visible, setVisible] = useState(false);
  const cssScale = scale * POINTS_TO_CSS_PX;

  useEffect(() => {
    const frame = frameRef.current;
    if (!frame || !root) return;
    const observer = new IntersectionObserver(
      ([entry]) => setVisible(Boolean(entry?.isIntersecting)),
      {
        root,
        rootMargin: "100% 0px",
      },
    );
    observer.observe(frame);
    return () => observer.disconnect();
  }, [root]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !visible) return;
    let task: ReturnType<Awaited<ReturnType<PDFDocumentProxy["getPage"]>>["render"]> | undefined;
    let cancelled = false;

    void pdf.getPage(pageNumber).then((page) => {
      if (cancelled) return;
      const viewport = page.getViewport({ scale: cssScale * window.devicePixelRatio });
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);
      task = page.render({ canvas, viewport });
      task.promise.catch(() => undefined);
    });

    return () => {
      cancelled = true;
      task?.cancel();
      // Release the bitmap once the page scrolls far away.
      canvas.width = 0;
      canvas.height = 0;
    };
  }, [pdf, pageNumber, cssScale, visible]);

  return (
    <div
      ref={frameRef}
      data-page={pageNumber}
      aria-label={`Page ${pageNumber}`}
      role="img"
      className="relative h-(--page-height) w-(--page-width) shrink-0 bg-white shadow-(--paper-shadow)"
      style={
        {
          "--page-width": `${size.width * cssScale}px`,
          "--page-height": `${size.height * cssScale}px`,
        } as React.CSSProperties
      }
    >
      <canvas ref={canvasRef} className="absolute inset-0 size-full" />
    </div>
  );
}

type PrintPreviewProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  /** Null while the documents are still being gathered. */
  packet: PrintPacket | null;
  /** Secondary line under the title, e.g. how many documents are included. */
  description: string;
  onPrint: () => void;
};

export function PrintPreview({
  open,
  onOpenChange,
  title,
  packet,
  description,
  onPrint,
}: PrintPreviewProps) {
  // Focus lands on the pages so PageUp/PageDown and arrow scrolling work straight away.
  const pagesRef = useRef<HTMLDivElement | null>(null);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent size="workspace" showCloseButton={false} initialFocus={pagesRef}>
        <PrintPreviewBody
          pagesRef={pagesRef}
          title={title}
          packet={packet}
          description={description}
          onCancel={() => onOpenChange(false)}
          onPrint={onPrint}
        />
      </DialogContent>
    </Dialog>
  );
}

function PrintPreviewBody({
  pagesRef,
  title,
  packet,
  description,
  onCancel,
  onPrint,
}: {
  pagesRef: React.RefObject<HTMLDivElement | null>;
  title: string;
  packet: PrintPacket | null;
  description: string;
  onCancel: () => void;
  onPrint: () => void;
}) {
  const [attempt, setAttempt] = useState(0);
  const state = usePrintPdf(packet, attempt);
  const [zoom, setZoom] = useState<Zoom>("fit");
  const [viewport, setViewport] = useState<HTMLDivElement | null>(null);
  const [viewportWidth, setViewportWidth] = useState(0);
  const [currentPage, setCurrentPage] = useState(1);
  const [pageDraft, setPageDraft] = useState<string | null>(null);
  // How far through the document the reader is, so zooming keeps the same spot in view.
  const scrollFractionRef = useRef(0);
  const pages = state._tag === "Ready" ? state.pages : [];
  const widestPage = Math.max(...pages.map((page) => page.width), 1);
  const fitScale = Math.min(
    1,
    Math.max(0.25, (viewportWidth - CANVAS_PADDING_PX * 2) / (widestPage * POINTS_TO_CSS_PX)),
  );
  const scale = zoom === "fit" ? fitScale : zoom / 100;
  const zoomPercent = Math.round(scale * 100);

  useEffect(() => {
    if (!viewport) return;
    const observer = new ResizeObserver(([entry]) =>
      setViewportWidth(entry?.contentRect.width ?? 0),
    );
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [viewport]);

  useShortcut("print", onPrint, { allowInOverlay: true, enabled: state._tag === "Ready" });

  function goToPage(pageNumber: number) {
    const target = viewport?.querySelector<HTMLElement>(`[data-page="${pageNumber}"]`);
    if (!target || !viewport) return;
    viewport.scrollTo({
      top: target.offsetTop - PAGE_GAP_PX,
      behavior: Math.abs(pageNumber - currentPage) > 2 ? "instant" : "smooth",
    });
  }

  function stepZoom(direction: 1 | -1) {
    const next =
      direction === 1
        ? ZOOM_STEPS.find((step) => step > zoomPercent)
        : ZOOM_STEPS.toReversed().find((step) => step < zoomPercent);
    if (next) setZoom(next);
  }

  useLayoutEffect(() => {
    // Through the ref: the element held in state is read-only to React Compiler.
    const element = pagesRef.current;
    if (element) element.scrollTop = scrollFractionRef.current * element.scrollHeight;
  }, [pagesRef, scale, viewport]);

  function trackCurrentPage() {
    if (!viewport) return;
    scrollFractionRef.current = viewport.scrollTop / Math.max(viewport.scrollHeight, 1);
    const probe = viewport.scrollTop + viewport.clientHeight / 3;
    const frames = viewport.querySelectorAll<HTMLElement>("[data-page]");
    for (const frame of frames) {
      if (frame.offsetTop + frame.offsetHeight >= probe) {
        setCurrentPage(Number(frame.dataset.page));
        return;
      }
    }
  }

  return (
    <div
      className="flex min-h-0 flex-1 flex-col"
      onKeyDown={(event) => {
        if (event.key === "PageDown") {
          event.preventDefault();
          goToPage(Math.min(currentPage + 1, pages.length));
        } else if (event.key === "PageUp") {
          event.preventDefault();
          goToPage(Math.max(currentPage - 1, 1));
        } else if ((event.ctrlKey || event.metaKey) && (event.key === "=" || event.key === "+")) {
          event.preventDefault();
          stepZoom(1);
        } else if ((event.ctrlKey || event.metaKey) && event.key === "-") {
          event.preventDefault();
          stepZoom(-1);
        } else if ((event.ctrlKey || event.metaKey) && event.key === "0") {
          event.preventDefault();
          setZoom("fit");
        }
      }}
    >
      <header className="flex h-12 shrink-0 items-center gap-3 border-b ps-4 pe-2">
        <div className="flex min-w-0 items-baseline gap-2">
          <DialogTitle>Print preview</DialogTitle>
          <DialogDescription>
            <bdi>{title}</bdi> · {description}
            {state._tag === "Ready"
              ? ` · ${pages.length} ${pages.length === 1 ? "page" : "pages"}`
              : null}
          </DialogDescription>
        </div>
        <div className="ms-auto flex items-center gap-1">
          <Button variant="subtle" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <Hint label="Print" shortcut="print">
            <Button size="sm" disabled={state._tag !== "Ready"} onClick={onPrint}>
              <HugeiconsIcon icon={PrinterIcon} strokeWidth={2} data-icon="inline-start" />
              Print
            </Button>
          </Hint>
        </div>
      </header>

      <div
        ref={(element) => {
          pagesRef.current = element;
          setViewport(element);
        }}
        tabIndex={-1}
        className="relative min-h-0 flex-1 overflow-auto bg-canvas outline-none"
        onScroll={trackCurrentPage}
      >
        {state._tag === "Ready" ? (
          <div className="flex min-w-fit flex-col items-center gap-6 px-12 py-9">
            {pages.map((size, index) => (
              <PreviewPage
                key={index}
                pdf={state.pdf}
                pageNumber={index + 1}
                size={size}
                scale={scale}
                root={viewport}
              />
            ))}
          </div>
        ) : state._tag === "Failed" ? (
          <Empty className="h-full">
            <EmptyHeader>
              <EmptyTitle>Couldn't prepare the preview</EmptyTitle>
              <EmptyDescription>{state.message}</EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              <Button variant="outline" size="sm" onClick={() => setAttempt((value) => value + 1)}>
                Try again
              </Button>
            </EmptyContent>
          </Empty>
        ) : (
          <div
            className="flex h-full items-center justify-center gap-2 text-sm text-muted-foreground"
            role="status"
          >
            <Spinner />
            Laying out pages…
          </div>
        )}
      </div>

      <footer className="flex h-10 shrink-0 items-center gap-1 border-t px-2 text-xs text-muted-foreground tabular-nums">
        <IconAction
          label="Previous page"
          side="top"
          disabled={currentPage <= 1 || state._tag !== "Ready"}
          onClick={() => goToPage(currentPage - 1)}
        >
          <HugeiconsIcon icon={ArrowLeft01Icon} strokeWidth={2} />
        </IconAction>
        <span className="flex items-center gap-1.5 px-1">
          Page
          <Input
            variant="numeric"
            aria-label="Go to page"
            inputMode="numeric"
            disabled={state._tag !== "Ready"}
            value={pageDraft ?? String(currentPage)}
            onFocus={(event) => event.currentTarget.select()}
            onChange={(event) => setPageDraft(event.target.value.replace(/\D/g, ""))}
            onBlur={() => setPageDraft(null)}
            onKeyDown={(event) => {
              if (event.key !== "Enter") return;
              const target = Number(pageDraft);
              if (target >= 1 && target <= pages.length) goToPage(target);
              event.currentTarget.blur();
            }}
          />
          of {pages.length || "—"}
        </span>
        <IconAction
          label="Next page"
          side="top"
          disabled={currentPage >= pages.length || state._tag !== "Ready"}
          onClick={() => goToPage(currentPage + 1)}
        >
          <HugeiconsIcon icon={ArrowRight01Icon} strokeWidth={2} />
        </IconAction>

        <div className="ms-auto flex items-center gap-1">
          <IconAction label="Zoom out" side="top" onClick={() => stepZoom(-1)}>
            <HugeiconsIcon icon={MinusSignIcon} strokeWidth={2} />
          </IconAction>
          <Select<Zoom>
            value={zoom}
            onValueChange={(value) => {
              if (value) setZoom(value);
            }}
            items={ZOOM_ITEMS}
          >
            <SelectTrigger size="sm" aria-label="Zoom" className="w-28">
              <SelectValue>
                {() => (zoom === "fit" ? `Fit · ${zoomPercent}%` : `${zoom}%`)}
              </SelectValue>
            </SelectTrigger>
            <SelectContent align="end" alignItemWithTrigger={false} side="top">
              <SelectGroup>
                {ZOOM_ITEMS.map((item) => (
                  <SelectItem key={item.value} value={item.value}>
                    {item.label}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          <IconAction label="Zoom in" side="top" onClick={() => stepZoom(1)}>
            <HugeiconsIcon icon={PlusSignIcon} strokeWidth={2} />
          </IconAction>
        </div>
      </footer>
    </div>
  );
}

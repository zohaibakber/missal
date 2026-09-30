import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
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
import {
  PDF_CANVAS_RENDER_CONCURRENCY,
  PDF_PAGE_METADATA_CONCURRENCY,
  createPermitPool,
  forEachWithConcurrency,
  type PermitPool,
} from "#/components/print-preview-schedule";

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
  | {
      readonly _tag: "Ready";
      readonly pdf: PDFDocumentProxy;
      readonly pages: readonly (PageSize | null)[];
    }
  | { readonly _tag: "Failed"; readonly message: string };

function destroyTask(task?: PDFDocumentLoadingTask) {
  if (!task || task.destroyed) return;
  // The loading task owns the document and its worker.
  void task.destroy().catch(() => undefined);
}

function usePrintPdf(
  packet: PrintPacket | null,
  attempt: number,
  ownerId: string,
  active: boolean,
) {
  const [state, setState] = useState<PreviewState>({ _tag: "Loading" });

  useEffect(() => {
    if (!packet || !active) return;
    const abort = new AbortController();
    const requestId = crypto.randomUUID();
    let task: PDFDocumentLoadingTask | undefined;
    let published = false;
    setState({ _tag: "Loading" });

    void (async () => {
      try {
        const api = window.electronPrint;
        if (!api) throw new Error("Print preview is only available in the Missal desktop app.");
        const [rendered, pdfjs] = await Promise.all([
          api.renderPdf({ ownerId, requestId, html: packet.html }),
          import("pdfjs-dist"),
        ]);
        if (abort.signal.aborted || rendered._tag === "Ignored") return;
        if (rendered._tag === "Unavailable") {
          setState({ _tag: "Failed", message: rendered.message });
          return;
        }
        pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;
        task = pdfjs.getDocument({ data: rendered.pdf });
        const pdf = await task.promise;
        if (abort.signal.aborted) {
          destroyTask(task);
          return;
        }
        published = true;
        setState({ _tag: "Ready", pdf, pages: Array.from({ length: pdf.numPages }, () => null) });
        await forEachWithConcurrency({
          count: pdf.numPages,
          concurrency: PDF_PAGE_METADATA_CONCURRENCY,
          signal: abort.signal,
          worker: async (index) => {
            const viewport = (await pdf.getPage(index + 1)).getViewport({ scale: 1 });
            return { width: viewport.width, height: viewport.height };
          },
          onItem: (index, size) => {
            setState((current) => {
              if (current._tag !== "Ready" || current.pdf !== pdf) return current;
              return { ...current, pages: current.pages.with(index, size) };
            });
          },
        });
      } catch (error: unknown) {
        if (abort.signal.aborted) return;
        setState({
          _tag: "Failed",
          message: error instanceof Error ? error.message : "The pages could not be prepared.",
        });
      }
    })();

    return () => {
      abort.abort();
      window.electronPrint?.cancelPreview({ ownerId, requestId });
      if (!published) destroyTask(task);
    };
  }, [packet, attempt, ownerId, active]);

  const readyPdf = state._tag === "Ready" ? state.pdf : null;
  useEffect(() => {
    if (!readyPdf) return;
    return () => destroyTask(readyPdf.loadingTask);
  }, [readyPdf]);

  return state;
}

function PreviewPage({
  pdf,
  pageNumber,
  size,
  scale,
  observe,
  pool,
}: {
  pdf: PDFDocumentProxy;
  pageNumber: number;
  size: PageSize;
  scale: number;
  observe: (element: HTMLElement, onVisible: (visible: boolean) => void) => () => void;
  pool: PermitPool;
}) {
  const frameRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [visible, setVisible] = useState(false);
  const cssScale = scale * POINTS_TO_CSS_PX;

  useEffect(() => {
    const frame = frameRef.current;
    if (!frame) return;
    return observe(frame, setVisible);
  }, [observe]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !visible) return;
    const abort = new AbortController();
    let task: ReturnType<Awaited<ReturnType<PDFDocumentProxy["getPage"]>>["render"]> | undefined;

    void (async () => {
      try {
        await pool.acquire(abort.signal);
      } catch {
        return;
      }
      try {
        const page = await pdf.getPage(pageNumber);
        if (abort.signal.aborted) return;
        const viewport = page.getViewport({ scale: cssScale * window.devicePixelRatio });
        canvas.width = Math.floor(viewport.width);
        canvas.height = Math.floor(viewport.height);
        task = page.render({ canvas, viewport });
        await task.promise;
      } catch {
        // A cancelled or superseded render rejects. Leave the canvas blank.
      } finally {
        pool.release();
      }
    })();

    return () => {
      abort.abort();
      task?.cancel();
      // Frees the bitmap of a page scrolled out of view.
      canvas.width = 0;
      canvas.height = 0;
    };
  }, [pdf, pageNumber, cssScale, visible, pool]);

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
  ownerId: string;
  title: string;
  packet: PrintPacket | null;
  description: string;
  onPrint: () => void;
};

export function PrintPreview({
  open,
  onOpenChange,
  ownerId,
  title,
  packet,
  description,
  onPrint,
}: PrintPreviewProps) {
  const pagesRef = useRef<HTMLDivElement | null>(null);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent size="workspace" showCloseButton={false} initialFocus={pagesRef}>
        <PrintPreviewBody
          pagesRef={pagesRef}
          active={open}
          ownerId={ownerId}
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
  active,
  ownerId,
  title,
  packet,
  description,
  onCancel,
  onPrint,
}: {
  pagesRef: React.RefObject<HTMLDivElement | null>;
  active: boolean;
  ownerId: string;
  title: string;
  packet: PrintPacket | null;
  description: string;
  onCancel: () => void;
  onPrint: () => void;
}) {
  const [attempt, setAttempt] = useState(0);
  const renderPool = useRef(createPermitPool(PDF_CANVAS_RENDER_CONCURRENCY));
  const state = usePrintPdf(packet, attempt, ownerId, active);
  const [zoom, setZoom] = useState<Zoom>("fit");
  const [viewport, setViewport] = useState<HTMLDivElement | null>(null);
  const [viewportWidth, setViewportWidth] = useState(0);
  const [currentPage, setCurrentPage] = useState(1);
  const [pageDraft, setPageDraft] = useState<string | null>(null);
  const scrollFractionRef = useRef(0);
  const pageCallbacks = useRef(new Map<Element, (visible: boolean) => void>());
  const pageObserver = useRef<IntersectionObserver | null>(null);
  const pages = state._tag === "Ready" ? state.pages : [];
  const measuredPages = pages.flatMap((page) => (page ? [page] : []));
  const hasMeasuredPage = measuredPages.length > 0;
  const widestPage = Math.max(...measuredPages.map((page) => page.width), 1);
  const fitScale = Math.min(
    1,
    Math.max(0.25, (viewportWidth - CANVAS_PADDING_PX * 2) / (widestPage * POINTS_TO_CSS_PX)),
  );
  const scale = zoom === "fit" ? fitScale : zoom / 100;
  const zoomPercent = Math.round(scale * 100);

  const observePage = useCallback((element: HTMLElement, onVisible: (visible: boolean) => void) => {
    pageCallbacks.current.set(element, onVisible);
    pageObserver.current?.observe(element);
    return () => {
      pageCallbacks.current.delete(element);
      pageObserver.current?.unobserve(element);
    };
  }, []);

  const setViewportElement = useCallback(
    (element: HTMLDivElement | null) => {
      pagesRef.current = element;
      setViewport(element);
      pageObserver.current?.disconnect();
      pageObserver.current = null;
      if (!element) return;
      // One observer for the whole preview, created with the scroll root before page effects run.
      const observer = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            pageCallbacks.current.get(entry.target)?.(entry.isIntersecting);
          }
        },
        { root: element, rootMargin: "100% 0px" },
      );
      pageObserver.current = observer;
      for (const node of pageCallbacks.current.keys()) observer.observe(node);
    },
    [pagesRef],
  );

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
        ref={setViewportElement}
        tabIndex={-1}
        className="relative min-h-0 flex-1 overflow-auto bg-canvas outline-none"
        onScroll={trackCurrentPage}
      >
        {state._tag === "Ready" && hasMeasuredPage ? (
          <div className="flex min-w-fit flex-col items-center gap-6 px-12 py-9">
            {pages.map((size, index) =>
              size ? (
                <PreviewPage
                  key={index}
                  pdf={state.pdf}
                  pageNumber={index + 1}
                  size={size}
                  scale={scale}
                  observe={observePage}
                  pool={renderPool.current}
                />
              ) : (
                <div
                  key={index}
                  data-page={index + 1}
                  aria-busy="true"
                  aria-label={`Page ${index + 1}`}
                  className="h-32 w-[min(100%,36rem)] shrink-0 bg-white/80"
                />
              ),
            )}
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

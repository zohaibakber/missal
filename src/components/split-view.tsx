import type { ComponentProps, ReactNode } from "react";
import { useDefaultLayout } from "react-resizable-panels";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "#/components/ui/resizable";
import { cn } from "#/lib/utils";

const LIST_PANEL_ID = "list";
const DETAIL_PANEL_ID = "detail";

function SplitView({ id, children }: { id: string; children: ReactNode }) {
  const layout = useDefaultLayout({
    id: `split-view:${id}`,
    panelIds: [LIST_PANEL_ID, DETAIL_PANEL_ID],
    storage: localStorage,
  });

  return (
    <ResizablePanelGroup
      orientation="horizontal"
      defaultLayout={layout.defaultLayout}
      onLayoutChanged={layout.onLayoutChanged}
      className="min-h-0 flex-1"
    >
      {children}
    </ResizablePanelGroup>
  );
}

function SplitViewList({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <>
      <ResizablePanel
        id={LIST_PANEL_ID}
        defaultSize={264}
        minSize={208}
        maxSize={440}
        groupResizeBehavior="preserve-pixel-size"
      >
        <div data-slot="split-view-list" className={cn("flex h-full min-h-0 flex-col", className)}>
          {children}
        </div>
      </ResizablePanel>
      <ResizableHandle />
    </>
  );
}

function SplitViewDetail({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <ResizablePanel id={DETAIL_PANEL_ID} minSize="40">
      <div
        data-slot="split-view-detail"
        className={cn("flex h-full min-h-0 min-w-0 flex-col", className)}
      >
        {children}
      </div>
    </ResizablePanel>
  );
}

function SplitViewListHeader({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="split-view-list-header"
      className={cn("flex h-12 shrink-0 items-center gap-2 border-b ps-4 pe-2", className)}
      {...props}
    />
  );
}

function SplitViewListTitle({ className, ...props }: ComponentProps<"h2">) {
  return (
    <h2
      data-slot="split-view-list-title"
      className={cn(
        "flex min-w-0 flex-1 items-baseline gap-2 truncate text-sm font-medium tracking-tight",
        className,
      )}
      {...props}
    />
  );
}

export { SplitView, SplitViewDetail, SplitViewList, SplitViewListHeader, SplitViewListTitle };

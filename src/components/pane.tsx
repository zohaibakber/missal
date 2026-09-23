import { createContext, use, useState, type ComponentProps, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cn } from "#/lib/utils";

/** A content surface inside the window: toolbar header, scrolling body, optional status bar. */
function Pane({ className, ...props }: ComponentProps<"section">) {
  return (
    <section
      data-slot="pane"
      className={cn("flex h-full min-h-0 min-w-0 flex-col", className)}
      {...props}
    />
  );
}

function PaneHeader({ className, ...props }: ComponentProps<"header">) {
  return (
    <header
      data-slot="pane-header"
      className={cn("flex h-12 shrink-0 items-center gap-2 border-b ps-4 pe-2", className)}
      {...props}
    />
  );
}

function PaneTitle({ className, ...props }: ComponentProps<"h1">) {
  return (
    <h1
      data-slot="pane-title"
      className={cn("truncate text-sm font-medium tracking-tight", className)}
      {...props}
    />
  );
}

/** Quiet secondary text next to the title, e.g. a count. */
function PaneMeta({ className, ...props }: ComponentProps<"span">) {
  return (
    <span
      data-slot="pane-meta"
      className={cn("text-sm text-muted-foreground tabular-nums", className)}
      {...props}
    />
  );
}

function PaneActions({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="pane-actions"
      className={cn("ms-auto flex shrink-0 items-center gap-1", className)}
      {...props}
    />
  );
}

function PaneBody({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="pane-body"
      className={cn("min-h-0 flex-1 overflow-auto", className)}
      {...props}
    />
  );
}

/** Slim status bar pinned to the bottom of the pane. */
function PaneStatusBar({ className, ...props }: ComponentProps<"footer">) {
  return (
    <footer
      data-slot="pane-status-bar"
      className={cn(
        "flex h-8 shrink-0 items-center gap-3 border-t px-4 text-xs text-muted-foreground tabular-nums",
        className,
      )}
      {...props}
    />
  );
}

function SaveStatus({ dirty, className }: { dirty: boolean; className?: string }) {
  return (
    <span
      role="status"
      data-dirty={dirty}
      className={cn(
        "flex items-center gap-1.5 px-2 text-xs text-muted-foreground data-[dirty=true]:text-foreground",
        className,
      )}
    >
      <span
        aria-hidden="true"
        className="size-1.5 rounded-full bg-muted-foreground/40 in-data-[dirty=true]:bg-status-open"
      />
      {dirty ? "Unsaved" : "Saved"}
    </span>
  );
}

type ActionsSlot = {
  target: HTMLElement | null;
  setTarget: (element: HTMLElement | null) => void;
};

const PaneActionsSlotContext = createContext<ActionsSlot | null>(null);

/**
 * Lets content deep inside a pane (e.g. the active tab's form) place its actions in the
 * pane header: render `PaneActionsOutlet` in the header and `PaneActionsPortal` anywhere below.
 */
function PaneActionsProvider({ children }: { children: ReactNode }) {
  const [target, setTarget] = useState<HTMLElement | null>(null);
  return <PaneActionsSlotContext value={{ target, setTarget }}>{children}</PaneActionsSlotContext>;
}

function PaneActionsOutlet(props: Omit<ComponentProps<typeof PaneActions>, "ref">) {
  const slot = use(PaneActionsSlotContext);
  return <PaneActions ref={slot?.setTarget} {...props} />;
}

function PaneActionsPortal({ children }: { children: ReactNode }) {
  const target = use(PaneActionsSlotContext)?.target;
  return target ? createPortal(children, target) : null;
}

export {
  Pane,
  PaneActions,
  PaneActionsOutlet,
  PaneActionsPortal,
  PaneActionsProvider,
  PaneBody,
  PaneHeader,
  PaneMeta,
  PaneStatusBar,
  PaneTitle,
  SaveStatus,
};

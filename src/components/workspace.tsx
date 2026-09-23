import type { ComponentProps } from "react";
import { cn } from "#/lib/utils";

export function WorkspaceHeader({ className, ...props }: ComponentProps<"header">) {
  return (
    <header
      data-slot="workspace-header"
      className={cn(
        "flex min-h-12 shrink-0 flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b px-4 py-2",
        className,
      )}
      {...props}
    />
  );
}

export function SaveStatus({ dirty }: { dirty: boolean }) {
  return (
    <span
      role="status"
      data-dirty={dirty}
      className="text-xs text-muted-foreground data-[dirty=true]:text-foreground"
    >
      {dirty ? "Unsaved changes" : "Saved"}
    </span>
  );
}

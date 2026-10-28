import type { ComponentProps, ReactNode } from "react";
import { Sidebar } from "#/components/ui/sidebar";
import { cn } from "#/lib/utils";

function RailLayout({
  rail,
  label,
  children,
}: {
  rail: ReactNode;
  label: string;
  children: ReactNode;
}) {
  return (
    <div data-slot="rail-layout" className="flex min-h-0 flex-1">
      <div data-slot="rail-layout-main" className="flex min-h-0 min-w-0 flex-1 flex-col">
        {children}
      </div>
      <Sidebar
        collapsible="none"
        aria-label={label}
        className="h-auto min-h-0 w-72 shrink-0 border-s"
      >
        {rail}
      </Sidebar>
    </div>
  );
}

function RailHeader({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="rail-header"
      className={cn("flex h-10 shrink-0 items-center gap-2 ps-4 pe-1.5", className)}
      {...props}
    />
  );
}

function RailTitle({ className, ...props }: ComponentProps<"h2">) {
  return (
    <h2
      data-slot="rail-title"
      className={cn("min-w-0 flex-1 truncate text-xs font-medium text-muted-foreground", className)}
      {...props}
    />
  );
}

export { RailHeader, RailLayout, RailTitle };

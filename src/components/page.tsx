import type { ComponentProps } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "#/lib/utils";

const pageVariants = cva("mx-auto flex w-full flex-col gap-6 p-6", {
  variants: {
    width: {
      wide: "max-w-7xl",
      narrow: "max-w-3xl",
    },
  },
  defaultVariants: {
    width: "wide",
  },
});

function Page({
  className,
  width,
  ...props
}: ComponentProps<"div"> & VariantProps<typeof pageVariants>) {
  return <div data-slot="page" className={cn(pageVariants({ width }), className)} {...props} />;
}

function PageHeader({ className, ...props }: ComponentProps<"header">) {
  return (
    <header
      data-slot="page-header"
      className={cn("flex flex-wrap items-end justify-between gap-x-6 gap-y-3", className)}
      {...props}
    />
  );
}

function PageHeading({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="page-heading"
      className={cn("flex min-w-0 flex-col gap-1", className)}
      {...props}
    />
  );
}

function PageTitle({ className, ...props }: ComponentProps<"h1">) {
  return (
    <h1
      data-slot="page-title"
      className={cn("font-heading text-xl font-semibold tracking-tight", className)}
      {...props}
    />
  );
}

function PageDescription({ className, ...props }: ComponentProps<"p">) {
  return (
    <p
      data-slot="page-description"
      className={cn("text-sm text-muted-foreground", className)}
      {...props}
    />
  );
}

function PageActions({ className, ...props }: ComponentProps<"div">) {
  return (
    <div data-slot="page-actions" className={cn("flex items-center gap-2", className)} {...props} />
  );
}

function PageFooter({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="page-footer"
      dir="ltr"
      className={cn(
        "sticky bottom-0 z-10 flex items-center justify-end gap-2 border-t bg-background py-3",
        className,
      )}
      {...props}
    />
  );
}

export { Page, PageActions, PageDescription, PageFooter, PageHeader, PageHeading, PageTitle };

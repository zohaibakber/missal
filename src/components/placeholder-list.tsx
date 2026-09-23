import type { ComponentProps, ReactNode } from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#/components/ui/table";
import { cn } from "#/lib/utils";

/** An editable two-column list of placeholders, used on the Placeholders screen. */
function PlaceholderList({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="placeholder-list"
      dir="rtl"
      lang="ur"
      className={cn("overflow-hidden rounded-lg border", className)}
      {...props}
    />
  );
}

function PlaceholderListTable({
  nameHeading,
  valueHeading,
  children,
}: {
  nameHeading: string;
  valueHeading: string;
  children: ReactNode;
}) {
  return (
    <Table className="table-fixed">
      <TableHeader>
        <TableRow>
          <TableHead>{nameHeading}</TableHead>
          <TableHead>{valueHeading}</TableHead>
          <TableHead columnWidth={48}>
            <span className="sr-only">Actions</span>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>{children}</TableBody>
    </Table>
  );
}

function PlaceholderListRow(props: ComponentProps<typeof TableRow>) {
  return <TableRow className="hover:bg-transparent" {...props} />;
}

function PlaceholderListFooter({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      dir="ltr"
      lang="en"
      className={cn("flex justify-end border-t p-1.5", className)}
      {...props}
    />
  );
}

/** Description line shown above a placeholder list. */
function PlaceholderListIntro({ className, ...props }: ComponentProps<"p">) {
  return <p className={cn("mb-4 text-sm text-muted-foreground", className)} {...props} />;
}

export {
  PlaceholderList,
  PlaceholderListFooter,
  PlaceholderListIntro,
  PlaceholderListRow,
  PlaceholderListTable,
  TableCell as PlaceholderListCell,
};

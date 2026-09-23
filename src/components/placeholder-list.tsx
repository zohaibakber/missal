import type { ComponentProps } from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#/components/ui/table";
import { cn } from "#/lib/utils";

function PlaceholderList({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="placeholder-list"
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
  children: React.ReactNode;
}) {
  return (
    <Table className="table-fixed">
      <TableHeader>
        <TableRow className="hover:bg-transparent">
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

function PlaceholderListFooter({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("border-t p-2", className)} {...props} />;
}

export {
  PlaceholderList,
  PlaceholderListFooter,
  PlaceholderListTable,
  TableCell as PlaceholderListCell,
  TableRow as PlaceholderListRow,
};

import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  type PaginationState,
  type SortingState,
  useReactTable,
} from "@tanstack/react-table";
import {
  ArrowLeft01Icon,
  ArrowLeftDoubleIcon,
  ArrowRight01Icon,
  ArrowRightDoubleIcon,
  ChevronDown,
  ChevronUp,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardFooter } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

interface DataTableProps<TData, TValue> {
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
}

export function DataTable<TData, TValue>({ columns, data }: DataTableProps<TData, TValue>) {
  const pageSize = 10;

  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: pageSize,
  });

  const [sorting, setSorting] = useState<SortingState>([
    {
      desc: false,
      id: "departureTime",
    },
  ]);

  const table = useReactTable({
    columns,
    data,
    enableSortingRemoval: false,
    getCoreRowModel: getCoreRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getSortedRowModel: getSortedRowModel(),
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
    state: {
      pagination,
      sorting,
    },
  });

  return (
    <Card className="w-full py-0 gap-0">
      <Table className="table-fixed">
        <TableHeader className="bg-muted">
          {table.getHeaderGroups().map((headerGroup) => (
            <TableRow className="hover:bg-transparent" key={headerGroup.id}>
              {headerGroup.headers.map((header) => {
                const columnSize = header.column.getSize();
                return (
                  <TableHead
                    key={header.id}
                    style={columnSize ? { width: `${columnSize}px` } : undefined}
                  >
                    {header.isPlaceholder ? null : header.column.getCanSort() ? (
                      <div
                        className="flex h-full cursor-pointer select-none items-center justify-between gap-2"
                        onClick={header.column.getToggleSortingHandler()}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            header.column.getToggleSortingHandler()?.(e);
                          }
                        }}
                        role="button"
                        tabIndex={0}
                      >
                        {flexRender(header.column.columnDef.header, header.getContext())}
                        {{
                          asc: (
                            <HugeiconsIcon
                              icon={ChevronUp}
                              aria-hidden="true"
                              strokeWidth={2}
                              className="size-4"
                            />
                          ),
                          desc: (
                            <HugeiconsIcon
                              icon={ChevronDown}
                              aria-hidden="true"
                              strokeWidth={2}
                              className="size-4"
                            />
                          ),
                        }[header.column.getIsSorted() as string] ?? null}
                      </div>
                    ) : (
                      flexRender(header.column.columnDef.header, header.getContext())
                    )}
                  </TableHead>
                );
              })}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {table.getRowModel().rows.length ? (
            table.getRowModel().rows.map((row) => (
              <TableRow data-state={row.getIsSelected() ? "selected" : undefined} key={row.id}>
                {row.getVisibleCells().map((cell) => (
                  <TableCell key={cell.id}>
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </TableCell>
                ))}
              </TableRow>
            ))
          ) : (
            <TableRow>
              <TableCell className="h-24 text-center" colSpan={columns.length}>
                No results.
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
      <CardFooter className="p-2">
        <div className="flex items-center justify-between gap-2 w-full">
          {/* Results range selector */}
          <div className="flex items-center gap-2 whitespace-nowrap">
            <p className="text-muted-foreground text-sm">Viewing</p>
            <Select
              onValueChange={(value) => {
                table.setPageIndex(Number(value) - 1);
              }}
              value={`${table.getState().pagination.pageIndex + 1}`}
            >
              <SelectTrigger
                aria-label="Select result range"
                className="w-fit min-w-none"
                size="sm"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {Array.from({ length: table.getPageCount() }, (_, i) => {
                    const start = i * table.getState().pagination.pageSize + 1;
                    const end = Math.min(
                      (i + 1) * table.getState().pagination.pageSize,
                      table.getRowCount(),
                    );
                    const pageNum = i + 1;
                    return (
                      <SelectItem key={pageNum} value={`${pageNum}`}>
                        {`${start}-${end}`}
                      </SelectItem>
                    );
                  })}
                </SelectGroup>
              </SelectContent>
            </Select>
            <p className="text-muted-foreground text-sm">
              of <strong className="font-medium text-foreground">{table.getRowCount()}</strong>{" "}
              results
            </p>
          </div>

          <div className="flex items-center gap-2">
            <Button
              aria-label="Go to first page"
              disabled={!table.getCanPreviousPage()}
              onClick={() => table.setPageIndex(0)}
              size="icon-sm"
              type="button"
              variant="outline"
            >
              <HugeiconsIcon
                data-icon="inline-start"
                icon={ArrowLeftDoubleIcon}
                strokeWidth={2}
                className="rtl:rotate-180"
              />
            </Button>
            <Button
              aria-label="Go to previous page"
              disabled={!table.getCanPreviousPage()}
              onClick={() => table.previousPage()}
              size="icon-sm"
              type="button"
              variant="outline"
            >
              <HugeiconsIcon
                aria-hidden="true"
                data-icon="inline-start"
                icon={ArrowLeft01Icon}
                strokeWidth={2}
                className="rtl:rotate-180"
              />
            </Button>
            <Button
              aria-label="Go to next page"
              disabled={!table.getCanNextPage()}
              onClick={() => table.nextPage()}
              size="icon-sm"
              type="button"
              variant="outline"
            >
              <HugeiconsIcon
                aria-hidden="true"
                data-icon="inline-end"
                icon={ArrowRight01Icon}
                strokeWidth={2}
                className="rtl:rotate-180"
              />
            </Button>
            <Button
              aria-label="Go to last page"
              disabled={!table.getCanNextPage()}
              onClick={() => table.setPageIndex(table.getPageCount() - 1)}
              size="icon-sm"
              type="button"
              variant="outline"
            >
              <HugeiconsIcon data-icon="inline-end" icon={ArrowRightDoubleIcon} strokeWidth={2} />
            </Button>
          </div>
        </div>
      </CardFooter>
    </Card>
  );
}

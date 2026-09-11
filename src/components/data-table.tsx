import { DataTableToolbar, type DataTableToolbarConfig } from "#/components/data-table-toolbar";
import { Button } from "#/components/ui/button";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#/components/ui/table";
import {
  ArrowDataTransferVerticalIcon,
  ArrowLeft01Icon,
  ArrowLeftDoubleIcon,
  ArrowRight01Icon,
  ArrowRightDoubleIcon,
  ChevronDown,
  ChevronUp,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  type ColumnDef,
  type ColumnFiltersState,
  type ColumnVisibilityState,
  type PaginationState,
  type RowData,
  type RowSelectionState,
  type SortingState,
  useTable,
} from "@tanstack/react-table";
import { useState } from "react";
import { features, type DataTableFeatures } from "#/components/data-table-features";

interface DataTableProps<TData extends RowData> {
  columns: ColumnDef<DataTableFeatures, TData>[];
  data: TData[];
  initialSorting?: SortingState;
  onRowClick?: (row: TData) => void;
  tableDir?: "ltr" | "rtl";
  tableLang?: string;
  toolbar?: DataTableToolbarConfig<TData>;
}

function isInteractiveElement(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return Boolean(
    target.closest('a,button,input,select,textarea,[role="button"],[data-no-row-click]'),
  );
}

export function DataTable<TData extends RowData>({
  columns,
  data,
  initialSorting,
  onRowClick,
  tableDir = "ltr",
  tableLang,
  toolbar,
}: DataTableProps<TData>) {
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [sorting, setSorting] = useState<SortingState>(
    initialSorting ?? [
      {
        desc: false,
        id: "id",
      },
    ],
  );
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [columnVisibility, setColumnVisibility] = useState<ColumnVisibilityState>({});
  const [globalFilter, setGlobalFilter] = useState("");
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({});

  const table = useTable({
    features,
    columns,
    data,
    globalFilterFn: "includesString",
    onColumnFiltersChange: setColumnFilters,
    onColumnVisibilityChange: setColumnVisibility,
    onGlobalFilterChange: setGlobalFilter,
    onPaginationChange: setPagination,
    onRowSelectionChange: setRowSelection,
    onSortingChange: setSorting,
    state: {
      columnFilters,
      columnVisibility,
      globalFilter,
      pagination,
      rowSelection,
      sorting,
    },
  });

  return (
    <div className="flex w-full flex-col">
      {toolbar ? <DataTableToolbar config={toolbar} table={table} /> : null}
      <div className="overflow-hidden rounded-md border" dir={tableDir} lang={tableLang}>
        <Table className="table-fixed">
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow className="hover:bg-transparent" key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  const columnSize = header.column.getSize();
                  const sortedDirection = header.column.getIsSorted();
                  return (
                    <TableHead
                      key={header.id}
                      style={columnSize ? { width: `${columnSize}px` } : undefined}
                    >
                      {header.isPlaceholder ? null : header.column.getCanSort() ? (
                        <div
                          className="group/table-sort flex h-full cursor-pointer select-none items-center justify-between gap-2"
                          onClick={header.column.getToggleSortingHandler()}
                          onKeyDown={(event) => {
                            if (event.key === "Enter" || event.key === " ") {
                              event.preventDefault();
                              header.column.getToggleSortingHandler()?.(event);
                            }
                          }}
                          role="button"
                          tabIndex={0}
                        >
                          <table.FlexRender header={header} />
                          <HugeiconsIcon
                            icon={
                              sortedDirection === "asc"
                                ? ChevronUp
                                : sortedDirection === "desc"
                                  ? ChevronDown
                                  : ArrowDataTransferVerticalIcon
                            }
                            aria-hidden="true"
                            strokeWidth={2}
                            className={
                              sortedDirection
                                ? "size-4"
                                : "size-4 opacity-0 transition-opacity group-hover/table-sort:opacity-60 group-focus-visible/table-sort:opacity-60"
                            }
                          />
                        </div>
                      ) : (
                        <table.FlexRender header={header} />
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
                <TableRow
                  data-state={row.getIsSelected() ? "selected" : undefined}
                  key={row.id}
                  onClick={(event) => {
                    if (!onRowClick || isInteractiveElement(event.target)) {
                      return;
                    }

                    onRowClick(row.original);
                  }}
                  onKeyDown={(event) => {
                    if (!onRowClick || isInteractiveElement(event.target)) {
                      return;
                    }

                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      onRowClick(row.original);
                    }
                  }}
                  role={onRowClick ? "link" : undefined}
                  tabIndex={onRowClick ? 0 : undefined}
                  className={onRowClick ? "cursor-pointer" : undefined}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      <table.FlexRender cell={cell} />
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
      </div>
      <div className="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-col gap-2 text-sm text-muted-foreground sm:flex-row sm:items-center">
          <p>
            {table.getFilteredSelectedRowModel().rows.length} of{" "}
            {table.getFilteredRowModel().rows.length} row(s) selected.
          </p>
          <div className="flex items-center gap-2 whitespace-nowrap">
            <span>Viewing</span>
            <Select
              onValueChange={(value) => {
                table.setPageIndex(Number(value) - 1);
              }}
              value={`${table.state.pagination.pageIndex + 1}`}
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
                    const start = i * table.state.pagination.pageSize + 1;
                    const end = Math.min(
                      (i + 1) * table.state.pagination.pageSize,
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
            <span>
              of <strong className="font-medium text-foreground">{table.getRowCount()}</strong>{" "}
              results
            </span>
          </div>
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
    </div>
  );
}

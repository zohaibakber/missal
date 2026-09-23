import { useRef, useState } from "react";
import { features, type DataTableFeatures } from "#/components/data-table-features";
import { IconAction } from "#/components/icon-action";
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
import { useShortcut } from "#/hooks/use-shortcut";
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
  type ReactTable,
  type RowData,
  type SortingState,
  useTable,
} from "@tanstack/react-table";

export type DataTableInstance<TData extends RowData> = ReactTable<DataTableFeatures, TData>;

const PAGE_SIZES = [10, 20, 50, 100] as const;

export function useDataTable<TData extends RowData>({
  columns,
  data,
  initialSorting = [],
}: {
  columns: ColumnDef<DataTableFeatures, TData>[];
  data: TData[];
  initialSorting?: SortingState;
}) {
  return useTable({
    features,
    columns,
    data,
    globalFilterFn: "includesString",
    initialState: {
      pagination: { pageIndex: 0, pageSize: 20 },
      sorting: initialSorting,
    },
  });
}

function isInteractiveElement(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    target.closest('a,button,input,select,textarea,[role="button"],[data-no-row-click]') !== null
  );
}

type DataTableProps<TData extends RowData> = {
  table: DataTableInstance<TData>;
  onRowActivate?: (row: TData) => void;
  dir?: "ltr" | "rtl";
  lang?: string;
};

export function DataTable<TData extends RowData>({
  table,
  onRowActivate,
  dir,
  lang,
}: DataTableProps<TData>) {
  const rows = table.getRowModel().rows;
  const bodyRef = useRef<HTMLTableSectionElement>(null);
  const [tabStopId, setTabStopId] = useState<string | null>(null);
  const tabStop = rows.some((row) => row.id === tabStopId) ? tabStopId : rows[0]?.id;
  const columnCount = table.getVisibleLeafColumns().length;

  function rowElements() {
    return [...(bodyRef.current?.querySelectorAll<HTMLElement>("tr[data-row-id]") ?? [])];
  }

  function focusedRowIndex() {
    const focused = document.activeElement?.closest<HTMLElement>("tr[data-row-id]");
    return focused ? rowElements().indexOf(focused) : -1;
  }

  function moveFocus(offset: 1 | -1) {
    const elements = rowElements();
    const current = focusedRowIndex();
    const next = current === -1 ? 0 : Math.min(Math.max(current + offset, 0), elements.length - 1);
    elements[next]?.focus();
  }

  useShortcut("nextRow", () => moveFocus(1));
  useShortcut("previousRow", () => moveFocus(-1));
  useShortcut("selectRow", () => rows[focusedRowIndex()]?.toggleSelected());

  return (
    <div className="overflow-hidden rounded-lg border" dir={dir} lang={lang}>
      <Table className="table-fixed">
        <TableHeader>
          {table.getHeaderGroups().map((headerGroup) => (
            <TableRow className="hover:bg-transparent" key={headerGroup.id}>
              {headerGroup.headers.map((header) => {
                const sorted = header.column.getIsSorted();
                return (
                  <TableHead columnWidth={header.column.getSize()} key={header.id}>
                    {header.isPlaceholder ? null : header.column.getCanSort() ? (
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="group/sort -mx-2 h-7"
                        onClick={header.column.getToggleSortingHandler()}
                      >
                        <table.FlexRender header={header} />
                        <HugeiconsIcon
                          icon={
                            sorted === "asc"
                              ? ChevronUp
                              : sorted === "desc"
                                ? ChevronDown
                                : ArrowDataTransferVerticalIcon
                          }
                          aria-hidden="true"
                          data-icon="inline-end"
                          className={
                            sorted
                              ? undefined
                              : "opacity-0 transition-opacity group-hover/sort:opacity-60 group-focus-visible/sort:opacity-60"
                          }
                        />
                      </Button>
                    ) : (
                      <table.FlexRender header={header} />
                    )}
                  </TableHead>
                );
              })}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody ref={bodyRef}>
          {rows.length ? (
            rows.map((row) => (
              <TableRow
                data-row-id={row.id}
                data-state={row.getIsSelected() ? "selected" : undefined}
                key={row.id}
                tabIndex={row.id === tabStop ? 0 : -1}
                aria-selected={row.getIsSelected()}
                className="data-[activatable=true]:cursor-pointer"
                data-activatable={Boolean(onRowActivate)}
                onFocus={() => setTabStopId(row.id)}
                onClick={(event) => {
                  if (onRowActivate && !isInteractiveElement(event.target)) {
                    onRowActivate(row.original);
                  }
                }}
                onKeyDown={(event) => {
                  if (event.target !== event.currentTarget) return;
                  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                    event.preventDefault();
                    moveFocus(event.key === "ArrowDown" ? 1 : -1);
                  } else if (onRowActivate && (event.key === "Enter" || event.key === " ")) {
                    event.preventDefault();
                    onRowActivate(row.original);
                  }
                }}
              >
                {row.getVisibleCells().map((cell) => (
                  <TableCell key={cell.id}>
                    <table.FlexRender cell={cell} />
                  </TableCell>
                ))}
              </TableRow>
            ))
          ) : (
            <TableRow className="hover:bg-transparent">
              <TableCell className="h-32 text-center" colSpan={columnCount} dir="ltr" lang="en">
                <span className="text-muted-foreground">No results.</span>
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  );
}

export function DataTablePagination<TData extends RowData>({
  table,
}: {
  table: DataTableInstance<TData>;
}) {
  const { pageIndex, pageSize } = table.state.pagination;
  const pageCount = Math.max(table.getPageCount(), 1);

  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 text-sm">
      <p className="text-muted-foreground tabular-nums">
        {table.getFilteredSelectedRowModel().rows.length} of{" "}
        {table.getFilteredRowModel().rows.length} row(s) selected
      </p>
      <div className="flex items-center gap-6">
        <div className="flex items-center gap-2">
          <span className="font-medium">Rows per page</span>
          <Select value={`${pageSize}`} onValueChange={(value) => table.setPageSize(Number(value))}>
            <SelectTrigger size="sm" aria-label="Rows per page" className="w-18">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {PAGE_SIZES.map((size) => (
                  <SelectItem key={size} value={`${size}`}>
                    {size}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>
        <span className="font-medium tabular-nums">
          Page {pageIndex + 1} of {pageCount}
        </span>
        <div className="flex items-center gap-1">
          {[
            {
              label: "First page",
              icon: ArrowLeftDoubleIcon,
              disabled: !table.getCanPreviousPage(),
              onClick: () => table.setPageIndex(0),
            },
            {
              label: "Previous page",
              icon: ArrowLeft01Icon,
              disabled: !table.getCanPreviousPage(),
              onClick: () => table.previousPage(),
            },
            {
              label: "Next page",
              icon: ArrowRight01Icon,
              disabled: !table.getCanNextPage(),
              onClick: () => table.nextPage(),
            },
            {
              label: "Last page",
              icon: ArrowRightDoubleIcon,
              disabled: !table.getCanNextPage(),
              onClick: () => table.setPageIndex(table.getPageCount() - 1),
            },
          ].map((control) => (
            <IconAction
              key={control.label}
              label={control.label}
              disabled={control.disabled}
              onClick={control.onClick}
              variant="outline"
            >
              <HugeiconsIcon icon={control.icon} strokeWidth={2} />
            </IconAction>
          ))}
        </div>
      </div>
    </div>
  );
}

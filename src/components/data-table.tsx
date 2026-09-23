import { useRef, useState, type ReactNode } from "react";
import { features, type DataTableFeatures } from "#/components/data-table-features";
import { Button } from "#/components/ui/button";
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "#/components/ui/context-menu";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#/components/ui/table";
import { useShortcut } from "#/hooks/use-shortcut";
import { ArrowDown01Icon, ArrowUp01Icon, ArrowUpDownIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  type ColumnDef,
  type ReactTable,
  type Row,
  type RowData,
  type SortingState,
  useTable,
} from "@tanstack/react-table";

export type DataTableInstance<TData extends RowData> = ReactTable<DataTableFeatures, TData>;

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
    initialState: { sorting: initialSorting },
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
  /** Items for the right-click menu of a row. */
  rowContextMenu?: (row: TData) => ReactNode;
  empty?: ReactNode;
  dir?: "ltr" | "rtl";
  lang?: string;
};

export function DataTable<TData extends RowData>({
  table,
  onRowActivate,
  rowContextMenu,
  empty,
  dir,
  lang,
}: DataTableProps<TData>) {
  const rows = table.getRowModel().rows;
  const bodyRef = useRef<HTMLTableSectionElement>(null);
  const [tabStopId, setTabStopId] = useState<string | null>(null);
  const tabStop = rows.some((row) => row.id === tabStopId) ? tabStopId : rows[0]?.id;
  const columnCount = table.getVisibleLeafColumns().length;
  const selecting = table.getIsSomeRowsSelected() || table.getIsAllRowsSelected();

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

  function renderRow(row: Row<DataTableFeatures, TData>) {
    const rowProps = {
      "data-row-id": row.id,
      "data-state": row.getIsSelected() ? "selected" : undefined,
      tabIndex: row.id === tabStop ? 0 : -1,
      "aria-selected": row.getIsSelected(),
      onFocus: () => setTabStopId(row.id),
      onClick: (event: React.MouseEvent) => {
        if (onRowActivate && !isInteractiveElement(event.target)) onRowActivate(row.original);
      },
      onKeyDown: (event: React.KeyboardEvent) => {
        if (event.target !== event.currentTarget) return;
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          moveFocus(event.key === "ArrowDown" ? 1 : -1);
        } else if (onRowActivate && (event.key === "Enter" || event.key === " ")) {
          event.preventDefault();
          onRowActivate(row.original);
        }
      },
    };
    const cells = row.getVisibleCells().map((cell) => (
      <TableCell key={cell.id}>
        <table.FlexRender cell={cell} />
      </TableCell>
    ));

    if (!rowContextMenu) {
      return (
        <TableRow key={row.id} {...rowProps}>
          {cells}
        </TableRow>
      );
    }

    return (
      <ContextMenu key={row.id}>
        <ContextMenuTrigger render={<TableRow {...rowProps} />}>{cells}</ContextMenuTrigger>
        <ContextMenuContent className="w-52">{rowContextMenu(row.original)}</ContextMenuContent>
      </ContextMenu>
    );
  }

  return (
    <div
      data-selecting={selecting}
      className="group/table min-h-0 flex-1 overflow-auto [&_[data-slot=table-container]]:overflow-visible"
      dir={dir}
      lang={lang}
    >
      <Table className="table-fixed">
        <TableHeader>
          {table.getHeaderGroups().map((headerGroup) => (
            <TableRow key={headerGroup.id}>
              {headerGroup.headers.map((header) => {
                const sorted = header.column.getIsSorted();
                return (
                  <TableHead
                    columnWidth={header.column.getSize()}
                    key={header.id}
                    aria-sort={
                      sorted === "asc" ? "ascending" : sorted === "desc" ? "descending" : undefined
                    }
                  >
                    {header.isPlaceholder ? null : header.column.getCanSort() ? (
                      <Button
                        type="button"
                        variant="subtle"
                        size="xs"
                        className="group/sort -mx-2"
                        onClick={header.column.getToggleSortingHandler()}
                      >
                        <table.FlexRender header={header} />
                        <HugeiconsIcon
                          icon={
                            sorted === "asc"
                              ? ArrowUp01Icon
                              : sorted === "desc"
                                ? ArrowDown01Icon
                                : ArrowUpDownIcon
                          }
                          strokeWidth={2}
                          aria-hidden="true"
                          data-icon="inline-end"
                          data-sorted={Boolean(sorted)}
                          className="opacity-0 transition-opacity group-hover/sort:opacity-60 group-focus-visible/sort:opacity-60 data-[sorted=true]:opacity-100"
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
            rows.map(renderRow)
          ) : (
            <TableRow className="hover:bg-transparent">
              <TableCell className="h-40" colSpan={columnCount} dir="ltr" lang="en">
                {empty ?? <p className="text-center text-muted-foreground">No results.</p>}
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  );
}

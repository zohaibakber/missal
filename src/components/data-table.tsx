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
import { useVirtualRows } from "#/hooks/use-virtual-rows";
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

// Every row is one h-10 line (cell text truncates, and Urdu's line height fits), so virtual rows
// use this fixed height instead of measuring each one.
const ROW_HEIGHT = 40;
const OVERSCAN_ROWS = 12;

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
  // State, not a ref: the rows' virtualizer reads it during layout, before a parent ref is attached.
  const [scrollElement, setScrollElement] = useState<HTMLDivElement | null>(null);
  const selecting = table.getIsSomeRowsSelected() || table.getIsAllRowsSelected();

  return (
    <div
      ref={setScrollElement}
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
        <DataTableRows
          table={table}
          scrollElement={scrollElement}
          onRowActivate={onRowActivate}
          rowContextMenu={rowContextMenu}
          empty={empty}
        />
      </Table>
    </div>
  );
}

type MenuRow<TData> = { readonly id: string; readonly original: TData };

function DataTableRows<TData extends RowData>({
  table,
  scrollElement,
  onRowActivate,
  rowContextMenu,
  empty,
}: Omit<DataTableProps<TData>, "dir" | "lang"> & {
  scrollElement: HTMLDivElement | null;
}) {
  const rows = table.getRowModel().rows;
  const bodyRef = useRef<HTMLTableSectionElement>(null);
  const [tabStopId, setTabStopId] = useState<string | null>(null);
  const [menuRow, setMenuRow] = useState<MenuRow<TData> | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const columnCount = table.getVisibleLeafColumns().length;
  const virtualizer = useVirtualRows({
    count: rows.length,
    scrollElement,
    rowHeight: ROW_HEIGHT,
    overscan: OVERSCAN_ROWS,
  });
  const virtualRows = virtualizer.items;
  const paddingTop = virtualRows[0]?.start ?? 0;
  const paddingBottom = virtualizer.totalSize - (virtualRows.at(-1)?.end ?? 0);
  const renderedIds = virtualRows.map((virtualRow) => rows[virtualRow.index]?.id);
  const tabStop = tabStopId && renderedIds.includes(tabStopId) ? tabStopId : renderedIds[0];

  function rowElement(index: number) {
    return bodyRef.current?.querySelector<HTMLElement>(`tr[data-index="${index}"]`) ?? null;
  }

  function focusedRowIndex() {
    const focused = document.activeElement?.closest<HTMLElement>("tr[data-index]");
    return focused && bodyRef.current?.contains(focused) ? Number(focused.dataset.index) : -1;
  }

  function focusRow(index: number) {
    const element = rowElement(index);
    if (element) {
      element.focus();
      return;
    }
    virtualizer.scrollToIndex(index);
    requestAnimationFrame(() => requestAnimationFrame(() => rowElement(index)?.focus()));
  }

  function moveFocus(offset: 1 | -1) {
    if (!rows.length) return;
    const current = focusedRowIndex();
    focusRow(current === -1 ? 0 : Math.min(Math.max(current + offset, 0), rows.length - 1));
  }

  useShortcut("nextRow", () => moveFocus(1));
  useShortcut("previousRow", () => moveFocus(-1));
  useShortcut("selectRow", () => rows[focusedRowIndex()]?.toggleSelected());

  function renderRow(row: Row<DataTableFeatures, TData>, index: number) {
    return (
      <TableRow
        key={row.id}
        data-index={index}
        data-row-id={row.id}
        data-state={row.getIsSelected() ? "selected" : undefined}
        data-popup-open={menuOpen && menuRow?.id === row.id ? "" : undefined}
        tabIndex={row.id === tabStop ? 0 : -1}
        aria-selected={row.getIsSelected()}
        onFocus={() => setTabStopId(row.id)}
        onClick={(event) => {
          if (onRowActivate && !isInteractiveElement(event.target)) onRowActivate(row.original);
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
    );
  }

  const content = rows.length ? (
    <>
      {paddingTop > 0 ? <Spacer height={paddingTop} columnCount={columnCount} /> : null}
      {virtualRows.map((virtualRow) => {
        const row = rows[virtualRow.index];
        return row ? renderRow(row, virtualRow.index) : null;
      })}
      {paddingBottom > 0 ? <Spacer height={paddingBottom} columnCount={columnCount} /> : null}
    </>
  ) : (
    <TableRow className="hover:bg-transparent">
      <TableCell className="h-40" colSpan={columnCount} dir="ltr" lang="en">
        {empty ?? <p className="text-center text-muted-foreground">No results.</p>}
      </TableCell>
    </TableRow>
  );

  if (!rowContextMenu) {
    return <TableBody ref={bodyRef}>{content}</TableBody>;
  }

  return (
    <ContextMenu
      open={menuOpen}
      onOpenChange={(open, details) => {
        if (open) {
          const target = details.event.target;
          const element =
            target instanceof Element ? target.closest<HTMLElement>("tr[data-row-id]") : null;
          const row = rows.find((candidate) => candidate.id === element?.dataset.rowId);
          if (!row) {
            details.cancel();
            return;
          }
          setMenuRow({ id: row.id, original: row.original });
        }
        setMenuOpen(open);
      }}
    >
      <ContextMenuTrigger render={<TableBody ref={bodyRef} />}>{content}</ContextMenuTrigger>
      <ContextMenuContent className="w-52">
        {menuRow ? rowContextMenu(menuRow.original) : null}
      </ContextMenuContent>
    </ContextMenu>
  );
}

function Spacer({ height, columnCount }: { height: number; columnCount: number }) {
  return (
    <tr aria-hidden="true">
      <td
        colSpan={columnCount}
        className="h-(--spacer-height) p-0"
        style={{ "--spacer-height": `${height}px` }}
      />
    </tr>
  );
}

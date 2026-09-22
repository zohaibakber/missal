import {
  columnFilteringFeature,
  columnSizingFeature,
  columnVisibilityFeature,
  constructFilterFn,
  createFilteredRowModel,
  createPaginatedRowModel,
  createSortedRowModel,
  filterFn_includesString,
  globalFilteringFeature,
  rowPaginationFeature,
  rowSelectionFeature,
  rowSortingFeature,
  sortFn_alphanumeric,
  sortFn_text,
  tableFeatures,
  type CellData,
  type RowData,
  type TableFeatures,
} from "@tanstack/react-table";

/** Keeps rows whose value is one of the selected options (used by faceted filters). */
const filterFn_oneOf = constructFilterFn({
  filter: (dataValue: unknown, filterValue: readonly unknown[]) => filterValue.includes(dataValue),
  autoRemove: (value: readonly unknown[] | undefined) => !value?.length,
});

export const features = tableFeatures({
  columnFilteringFeature,
  columnSizingFeature,
  columnVisibilityFeature,
  globalFilteringFeature,
  rowPaginationFeature,
  rowSelectionFeature,
  rowSortingFeature,
  filteredRowModel: createFilteredRowModel(),
  paginatedRowModel: createPaginatedRowModel(),
  sortedRowModel: createSortedRowModel(),
  filterFns: {
    includesString: filterFn_includesString,
    oneOf: filterFn_oneOf,
  },
  sortFns: {
    alphanumeric: sortFn_alphanumeric,
    text: sortFn_text,
  },
});

export type DataTableFeatures = typeof features;

declare module "@tanstack/react-table" {
  // oxlint-disable-next-line no-unused-vars -- generics must match the declaration being merged.
  interface ColumnMeta<
    TFeatures extends TableFeatures,
    TData extends RowData,
    TValue extends CellData,
  > {
    /** Plain-text column name for menus such as column visibility. */
    label?: string;
  }
}

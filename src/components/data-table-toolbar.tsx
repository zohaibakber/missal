"use no memo";

import type { Table } from "@tanstack/react-table";
import {
  Cancel01Icon,
  Eraser01Icon,
  FilterHorizontalIcon,
  FilterMailIcon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "#/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import { cn } from "#/lib/utils";

const ALL_OPTION_VALUE = "__all__";

export type DataTableToolbarFilterOption = {
  label: string;
  value: string;
};

export type DataTableToolbarFilterConfig = {
  columnId: string;
  label: string;
  options: DataTableToolbarFilterOption[];
  placeholder: string;
};

export type DataTableToolbarConfig<TData> = {
  filters?: DataTableToolbarFilterConfig[];
  search?: {
    label?: string;
    placeholder?: string;
    searchableColumnIds: Array<keyof TData | string>;
  };
};

interface DataTableToolbarProps<TData> {
  className?: string;
  config: DataTableToolbarConfig<TData>;
  table: Table<TData>;
}

function getColumnLabel(columnId: string) {
  return columnId
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/^./, (value) => value.toUpperCase());
}

export function DataTableToolbar<TData>({
  className,
  config,
  table,
}: DataTableToolbarProps<TData>) {
  const activeSearch = (table.getState().globalFilter as string) ?? "";
  const activeFilterCount = table
    .getState()
    .columnFilters.filter((filter) => filter.value !== undefined && filter.value !== "").length;
  const hasActiveFilters = activeFilterCount > 0;
  const hasActiveToolbarState = activeSearch.length > 0 || activeFilterCount > 0;
  const hideableColumns = table
    .getAllColumns()
    .filter((column) => typeof column.accessorFn !== "undefined" && column.getCanHide());

  return (
    <div className={cn("flex flex-col gap-3 border-b p-2 py-2.5 bg-muted", className)}>
      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        {config.search ? (
          <InputGroup className="w-full lg:max-w-xs bg-background">
            <InputGroupAddon align="inline-start">
              <HugeiconsIcon
                aria-hidden="true"
                className="text-muted-foreground"
                icon={Search01Icon}
                strokeWidth={2}
              />
            </InputGroupAddon>
            <InputGroupInput
              aria-label={config.search.label ?? "Search table data"}
              onChange={(event) => table.setGlobalFilter(event.target.value)}
              placeholder={config.search.placeholder ?? "Search"}
              value={activeSearch}
            />
            {activeSearch ? (
              <InputGroupAddon align="inline-end">
                <InputGroupButton
                  aria-label="Clear search"
                  onClick={() => table.setGlobalFilter("")}
                  size="icon-xs"
                >
                  <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
                </InputGroupButton>
              </InputGroupAddon>
            ) : null}
          </InputGroup>
        ) : (
          <div />
        )}

        <div className="flex flex-1 items-center justify-end gap-2">
          {config.filters?.length ? (
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button
                    aria-label="Open filters"
                    className="relative"
                    size="icon-sm"
                    type="button"
                    variant={"outline"}
                  />
                }
              >
                <HugeiconsIcon icon={FilterHorizontalIcon} strokeWidth={2} />
                {hasActiveFilters ? (
                  <span className="absolute -top-0.5 -inset-e-0.5 size-1.5 rounded-full bg-primary" />
                ) : null}
                <span className="sr-only">Open filters</span>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-48">
                <DropdownMenuGroup>
                  <DropdownMenuLabel>
                    {hasActiveFilters ? `Filters (${activeFilterCount})` : "Filters"}
                  </DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  {config.filters.map((filter) => {
                    const column = table.getColumn(filter.columnId);

                    if (!column) {
                      return null;
                    }

                    const value =
                      (column.getFilterValue() as string | undefined) ?? ALL_OPTION_VALUE;

                    return (
                      <DropdownMenuSub key={filter.columnId}>
                        <DropdownMenuSubTrigger>{filter.label}</DropdownMenuSubTrigger>
                        <DropdownMenuSubContent className="w-44">
                          <DropdownMenuGroup>
                            <DropdownMenuLabel>{filter.label}</DropdownMenuLabel>
                            <DropdownMenuSeparator />
                            <DropdownMenuRadioGroup
                              onValueChange={(nextValue) => {
                                column.setFilterValue(
                                  nextValue === ALL_OPTION_VALUE ? undefined : nextValue,
                                );
                              }}
                              value={value}
                            >
                              <DropdownMenuRadioItem value={ALL_OPTION_VALUE}>
                                {filter.placeholder}
                              </DropdownMenuRadioItem>
                              {filter.options.map((option) => (
                                <DropdownMenuRadioItem key={option.value} value={option.value}>
                                  {option.label}
                                </DropdownMenuRadioItem>
                              ))}
                            </DropdownMenuRadioGroup>
                          </DropdownMenuGroup>
                        </DropdownMenuSubContent>
                      </DropdownMenuSub>
                    );
                  })}
                  {hasActiveToolbarState ? (
                    <>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem
                        onClick={() => {
                          table.resetColumnFilters();
                          table.setGlobalFilter("");
                        }}
                        variant="destructive"
                      >
                        <HugeiconsIcon icon={Eraser01Icon} className="mt-px" />
                        Reset Filters
                      </DropdownMenuItem>
                    </>
                  ) : null}
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : null}

          {hideableColumns.length ? (
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button
                    aria-label="Open view options"
                    size="icon-sm"
                    type="button"
                    variant="outline"
                  />
                }
              >
                <HugeiconsIcon icon={FilterMailIcon} strokeWidth={2} />
                <span className="sr-only">Open view options</span>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-48">
                <DropdownMenuGroup>
                  <DropdownMenuLabel>Toggle Columns</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  {hideableColumns.map((column) => (
                    <DropdownMenuCheckboxItem
                      key={column.id}
                      checked={column.getIsVisible()}
                      onCheckedChange={() => column.toggleVisibility(!column.getIsVisible())}
                    >
                      {getColumnLabel(column.id)}
                    </DropdownMenuCheckboxItem>
                  ))}
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : null}
        </div>
      </div>
    </div>
  );
}

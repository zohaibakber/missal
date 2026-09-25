import { useRef, type ReactNode } from "react";
import type { RowData } from "@tanstack/react-table";
import {
  Cancel01Icon,
  FilterHorizontalIcon,
  LayoutThreeColumnIcon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import type { DataTableInstance } from "#/components/data-table";
import { Hint } from "#/components/hint";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Button } from "#/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "#/components/ui/command";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import { Popover, PopoverContent, PopoverTrigger } from "#/components/ui/popover";
import { useShortcut } from "#/hooks/use-shortcut";
import { cn } from "#/lib/utils";

type TableProp<TData extends RowData> = { table: DataTableInstance<TData> };

export function DataTableSearch<TData extends RowData>({
  table,
  placeholder = "Search…",
  className,
}: TableProp<TData> & { placeholder?: string; className?: string }) {
  const inputRef = useRef<HTMLInputElement>(null);
  const value = table.state.globalFilter ?? "";

  useShortcut("search", () => {
    inputRef.current?.focus();
    inputRef.current?.select();
  });

  return (
    <InputGroup className={cn("h-7 w-56", className)}>
      <InputGroupAddon align="inline-start">
        <HugeiconsIcon icon={Search01Icon} strokeWidth={2} />
      </InputGroupAddon>
      <InputGroupInput
        ref={inputRef}
        aria-label={placeholder}
        placeholder={placeholder}
        value={value}
        onChange={(event) => table.setGlobalFilter(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape" && value) {
            event.stopPropagation();
            table.setGlobalFilter("");
          }
        }}
      />
      <InputGroupAddon align="inline-end">
        {value ? (
          <InputGroupButton
            aria-label="Clear search"
            onClick={() => table.setGlobalFilter("")}
            size="icon-xs"
          >
            <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
          </InputGroupButton>
        ) : (
          <ShortcutKbd id="search" />
        )}
      </InputGroupAddon>
    </InputGroup>
  );
}

export type FacetedFilterOption = {
  label: ReactNode;
  value: string;
  icon?: ReactNode;
};

export function DataTableFacetedFilter<TData extends RowData>({
  table,
  columnId,
  title,
  options,
  dir,
}: TableProp<TData> & {
  columnId: string;
  title: string;
  options: FacetedFilterOption[];
  dir?: "ltr" | "rtl";
}) {
  const column = table.getColumn(columnId);
  if (!column) return null;

  const selected = new Set((column.getFilterValue() as string[] | undefined) ?? []);

  function toggle(value: string) {
    const next = new Set(selected);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    column?.setFilterValue(next.size ? [...next] : undefined);
  }

  return (
    <Popover>
      <Hint label={selected.size ? `${title} · ${selected.size} selected` : title}>
        <PopoverTrigger
          render={
            <Button
              variant={selected.size ? "secondary" : "subtle"}
              size="icon-sm"
              type="button"
              aria-label={title}
            />
          }
        >
          <HugeiconsIcon icon={FilterHorizontalIcon} strokeWidth={2} />
        </PopoverTrigger>
      </Hint>
      <PopoverContent className="w-56" align="start" dir={dir}>
        <Command>
          <CommandInput placeholder={title} />
          <CommandList>
            <CommandEmpty>No results found.</CommandEmpty>
            <CommandGroup>
              {options.map((option) => (
                <CommandItem
                  key={option.value}
                  value={option.value}
                  data-checked={selected.has(option.value)}
                  onSelect={() => toggle(option.value)}
                >
                  {option.icon}
                  <span className="truncate">{option.label}</span>
                </CommandItem>
              ))}
            </CommandGroup>
            {selected.size ? (
              <>
                <CommandSeparator />
                <CommandGroup>
                  <CommandItem
                    className="justify-center"
                    onSelect={() => column.setFilterValue(undefined)}
                  >
                    Clear filter
                  </CommandItem>
                </CommandGroup>
              </>
            ) : null}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}

export function DataTableResetFilters<TData extends RowData>({ table }: TableProp<TData>) {
  const isFiltered =
    table.state.columnFilters.length > 0 || Boolean(table.state.globalFilter?.length);
  if (!isFiltered) return null;

  return (
    <Button
      variant="subtle"
      size="sm"
      type="button"
      onClick={() => {
        table.resetColumnFilters();
        table.setGlobalFilter("");
      }}
    >
      Clear
    </Button>
  );
}

export function DataTableViewOptions<TData extends RowData>({ table }: TableProp<TData>) {
  const columns = table
    .getAllColumns()
    .filter((column) => typeof column.accessorFn !== "undefined" && column.getCanHide());
  if (!columns.length) return null;

  return (
    <DropdownMenu>
      <Hint label="Columns">
        <DropdownMenuTrigger
          render={<Button variant="subtle" size="icon-sm" type="button" aria-label="Columns" />}
        >
          <HugeiconsIcon icon={LayoutThreeColumnIcon} strokeWidth={2} />
        </DropdownMenuTrigger>
      </Hint>
      <DropdownMenuContent align="start" className="w-44">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Show columns</DropdownMenuLabel>
          {columns.map((column) => (
            <DropdownMenuCheckboxItem
              key={column.id}
              checked={column.getIsVisible()}
              onCheckedChange={(checked) => column.toggleVisibility(Boolean(checked))}
            >
              {typeof column.columnDef.meta?.label === "string"
                ? column.columnDef.meta.label
                : column.id}
            </DropdownMenuCheckboxItem>
          ))}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

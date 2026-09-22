import { useRef, type ComponentProps, type ReactNode } from "react";
import type { RowData } from "@tanstack/react-table";
import {
  Add01Icon,
  Cancel01Icon,
  LayoutThreeColumnIcon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import type { DataTableInstance } from "#/components/data-table";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Badge } from "#/components/ui/badge";
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
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import { Popover, PopoverContent, PopoverTrigger } from "#/components/ui/popover";
import { Separator } from "#/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "#/components/ui/tooltip";
import { useShortcut } from "#/hooks/use-shortcut";
import { cn } from "#/lib/utils";

type TableProp<TData extends RowData> = { table: DataTableInstance<TData> };

export function DataTableToolbar({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="data-table-toolbar"
      className={cn("flex flex-wrap items-center gap-2", className)}
      {...props}
    />
  );
}

/** Global search; press `/` anywhere on the page to focus it. */
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
    <InputGroup className={cn("h-8 w-full sm:w-72", className)}>
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

/** Multi-select column filter in a popover, following the shadcn data-table faceted filter pattern. */
export function DataTableFacetedFilter<TData extends RowData>({
  table,
  columnId,
  title,
  options,
}: TableProp<TData> & { columnId: string; title: string; options: FacetedFilterOption[] }) {
  const column = table.getColumn(columnId);
  if (!column) return null;

  const selected = new Set((column.getFilterValue() as string[] | undefined) ?? []);
  const counts = new Map<string, number>();
  for (const row of table.getCoreRowModel().rows) {
    const value = String(row.getValue(columnId));
    counts.set(value, (counts.get(value) ?? 0) + 1);
  }

  function toggle(value: string) {
    const next = new Set(selected);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    column?.setFilterValue(next.size ? [...next] : undefined);
  }

  return (
    <Popover>
      <PopoverTrigger render={<Button variant="outline" size="sm" type="button" />}>
        <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
        {title}
        {selected.size ? (
          <>
            <Separator orientation="vertical" className="mx-0.5 data-vertical:h-4" />
            <Badge variant="secondary">{selected.size} selected</Badge>
          </>
        ) : null}
      </PopoverTrigger>
      <PopoverContent className="w-56 p-0" align="start">
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
                  <span className="ms-auto font-mono text-xs text-muted-foreground">
                    {counts.get(option.value) ?? 0}
                  </span>
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
      variant="ghost"
      size="sm"
      type="button"
      onClick={() => {
        table.resetColumnFilters();
        table.setGlobalFilter("");
      }}
    >
      Reset
      <HugeiconsIcon icon={Cancel01Icon} data-icon="inline-end" />
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
      <Tooltip>
        <TooltipTrigger
          render={
            <DropdownMenuTrigger
              render={
                <Button variant="outline" size="icon-sm" type="button" aria-label="Columns" />
              }
            />
          }
        >
          <HugeiconsIcon icon={LayoutThreeColumnIcon} />
        </TooltipTrigger>
        <TooltipContent>Columns</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end" className="w-44">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Toggle columns</DropdownMenuLabel>
          <DropdownMenuSeparator />
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

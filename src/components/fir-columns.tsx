import { createContext, use } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { Delete02Icon, Edit02Icon, MoreHorizontalIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { type DataTableFeatures } from "#/components/data-table-features";
import { FirStatusBadge } from "#/components/fir-status-badge";
import { Button } from "#/components/ui/button";
import { Checkbox } from "#/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import { formatDate } from "#/lib/date";
import type { FirSummary } from "#/lib/fir";

export type FirRowActions = {
  edit: (fir: FirSummary) => void;
  remove: (firs: readonly FirSummary[]) => void;
};

export const FirRowActionsContext = createContext<FirRowActions | null>(null);

function useFirRowActions() {
  const actions = use(FirRowActionsContext);
  if (!actions) throw new Error("FIR columns must be rendered inside FirRowActionsContext.");
  return actions;
}

function FirRowMenu({ fir }: { fir: FirSummary }) {
  const { edit, remove } = useFirRowActions();

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            type="button"
            variant="subtle"
            size="icon-xs"
            tabIndex={-1}
            aria-label={`Actions for FIR ${fir.fir_no}`}
          />
        }
      >
        <HugeiconsIcon icon={MoreHorizontalIcon} strokeWidth={2} />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-44" dir="ltr">
        <DropdownMenuGroup>
          <DropdownMenuItem onClick={() => edit(fir)}>
            <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
            Edit details
          </DropdownMenuItem>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem onClick={() => remove([fir])} variant="destructive">
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
            Delete
          </DropdownMenuItem>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

const revealClassName =
  "flex opacity-0 transition-opacity group-hover/row:opacity-100 group-focus-within/row:opacity-100 group-data-[selecting=true]/table:opacity-100 has-aria-expanded:opacity-100 has-data-checked:opacity-100";

const columnHelper = createColumnHelper<DataTableFeatures, FirSummary>();

const dateCell = (value: string | null | undefined) => (
  <div dir="ltr" className="text-end tabular-nums">
    {value ? formatDate(value) : <span className="text-muted-foreground">—</span>}
  </div>
);

export const firColumns = columnHelper.columns([
  columnHelper.display({
    id: "select",
    enableGlobalFilter: false,
    enableHiding: false,
    enableSorting: false,
    size: 44,
    header: ({ table }) => {
      const isAllSelected = table.getIsAllRowsSelected();
      const isSomeSelected = table.getIsSomeRowsSelected();
      return (
        <span className={revealClassName}>
          <Checkbox
            aria-label="Select all"
            checked={isAllSelected}
            indeterminate={isSomeSelected && !isAllSelected}
            onCheckedChange={(value) => table.toggleAllRowsSelected(!!value)}
          />
        </span>
      );
    },
    cell: ({ row }) => (
      <span className={revealClassName}>
        <Checkbox
          aria-label="Select row"
          tabIndex={-1}
          checked={row.getIsSelected()}
          onCheckedChange={(value) => row.toggleSelected(!!value)}
        />
      </span>
    ),
  }),
  columnHelper.accessor("fir_no", {
    header: "ایف آئی آر نمبر",
    meta: { label: "FIR no." },
    size: 150,
    cell: ({ row }) => (
      <div dir="ltr" className="truncate text-end font-medium">
        {row.original.fir_no}
      </div>
    ),
  }),
  columnHelper.accessor("offence", {
    header: "جرم",
    meta: { label: "Offence" },
    size: 180,
    cell: ({ row }) => <div className="truncate text-ur">{row.original.offence}</div>,
  }),
  columnHelper.accessor("status", {
    header: "حالت",
    meta: { label: "Status" },
    size: 120,
    filterFn: "oneOf",
    cell: ({ row }) => <FirStatusBadge status={row.original.status} />,
  }),
  columnHelper.accessor("date", {
    header: "تاریخ ایف آئی آر",
    meta: { label: "FIR date" },
    size: 110,
    cell: ({ row }) => dateCell(row.original.date),
  }),
  columnHelper.accessor("incident_date", {
    header: "تاریخ وقوعہ",
    meta: { label: "Incident date" },
    size: 110,
    cell: ({ row }) => dateCell(row.original.incident_date),
  }),
  columnHelper.accessor("arrest_date", {
    header: "تاریخ گرفتاری",
    meta: { label: "Arrest date" },
    size: 110,
    cell: ({ row }) => dateCell(row.original.arrest_date),
  }),
  columnHelper.display({
    id: "actions",
    enableGlobalFilter: false,
    enableHiding: false,
    enableSorting: false,
    size: 48,
    cell: ({ row }) => (
      <div
        data-no-row-click
        className={revealClassName}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => event.stopPropagation()}
      >
        <FirRowMenu fir={row.original} />
      </div>
    ),
  }),
]);

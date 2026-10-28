import { createContext, use } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { Delete02Icon, Edit02Icon, MoreHorizontalIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { type DataTableFeatures } from "#/components/data-table-features";
import { FirStatusLabel } from "#/components/fir-status-badge";
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
import { FIR_LABELS, type FirSummary } from "#/lib/fir";

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
      <DropdownMenuContent align="end" className="w-44">
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
  <div className="text-muted-foreground tabular-nums">
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
    header: FIR_LABELS.fir_no,
    meta: { label: FIR_LABELS.fir_no },
    size: 120,
    cell: ({ row }) => (
      <div className="truncate font-medium tabular-nums">{row.original.fir_no}</div>
    ),
  }),
  columnHelper.accessor("offence", {
    header: FIR_LABELS.offence,
    meta: { label: FIR_LABELS.offence, grow: true },
    cell: ({ row }) => (
      <div lang="ur" className="truncate text-ur">
        {row.original.offence}
      </div>
    ),
  }),
  columnHelper.accessor("status", {
    header: FIR_LABELS.status,
    meta: { label: FIR_LABELS.status },
    size: 172,
    filterFn: "oneOf",
    cell: ({ row }) => <FirStatusLabel status={row.original.status} className="max-w-full" />,
  }),
  columnHelper.accessor("date", {
    header: FIR_LABELS.date,
    meta: { label: FIR_LABELS.date },
    size: 124,
    cell: ({ row }) => dateCell(row.original.date),
  }),
  columnHelper.accessor("incident_date", {
    header: FIR_LABELS.incident_date,
    meta: { label: FIR_LABELS.incident_date },
    size: 124,
    cell: ({ row }) => dateCell(row.original.incident_date),
  }),
  columnHelper.accessor("arrest_date", {
    header: FIR_LABELS.arrest_date,
    meta: { label: FIR_LABELS.arrest_date },
    size: 124,
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

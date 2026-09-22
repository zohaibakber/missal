import { EditFirSheet } from "#/components/edit-fir-sheet";
import { FirStatusBadge } from "#/components/fir-status-badge";
import { Button } from "#/components/ui/button";
import { Checkbox } from "#/components/ui/checkbox";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "#/components/ui/alert-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import { type DataTableFeatures } from "#/components/data-table-features";
import { formatDate } from "#/lib/date";
import type { FirRecord } from "#/lib/fir";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { Exit } from "effect";
import { atoms } from "#/state/atoms";
import { Delete02Icon, Edit02Icon, MoreVerticalIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useAtomSet } from "@effect/atom-react";
import { createColumnHelper } from "@tanstack/react-table";
import { useState } from "react";
import { toast } from "#/components/ui/toast";

function FirRowActions({ fir }: { fir: FirRecord }) {
  const [isEditOpen, setIsEditOpen] = useState(false);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const removeFir = useAtomSet(atoms.removeFirAtom, { mode: "promiseExit" });

  async function handleDelete() {
    const exit = await removeFir(fir.id);

    if (Exit.isFailure(exit)) {
      toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
      return;
    }

    setIsDeleteDialogOpen(false);
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              tabIndex={-1}
              aria-label={`Actions for FIR ${fir.fir_no}`}
            />
          }
        >
          <HugeiconsIcon icon={MoreVerticalIcon} strokeWidth={2} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-40">
          <DropdownMenuItem onClick={() => setIsEditOpen(true)}>
            <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
            Edit details
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => setIsDeleteDialogOpen(true)} variant="destructive">
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <EditFirSheet fir={fir} open={isEditOpen} onOpenChange={setIsEditOpen} />
      <AlertDialog open={isDeleteDialogOpen} onOpenChange={setIsDeleteDialogOpen}>
        <AlertDialogContent size="sm">
          <AlertDialogHeader>
            <AlertDialogTitle>Delete FIR {fir.fir_no}?</AlertDialogTitle>
            <AlertDialogDescription>
              This removes the FIR and every document created for it. This can't be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(event) => {
                event.preventDefault();
                void handleDelete();
              }}
              type="button"
              variant="destructive"
            >
              <HugeiconsIcon data-icon="inline-start" icon={Delete02Icon} />
              Delete FIR
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

const columnHelper = createColumnHelper<DataTableFeatures, FirRecord>();

export const firColumns = columnHelper.columns([
  columnHelper.display({
    id: "select",
    enableGlobalFilter: false,
    enableHiding: false,
    enableSorting: false,
    size: 40,
    header: ({ table }) => {
      const isAllSelected = table.getIsAllPageRowsSelected();
      const isSomeSelected = table.getIsSomePageRowsSelected();
      return (
        <Checkbox
          aria-label="Select all rows"
          checked={isAllSelected}
          indeterminate={isSomeSelected && !isAllSelected}
          onCheckedChange={(value) => table.toggleAllPageRowsSelected(!!value)}
        />
      );
    },
    cell: ({ row }) => (
      <Checkbox
        aria-label="Select row"
        tabIndex={-1}
        checked={row.getIsSelected()}
        onCheckedChange={(value) => row.toggleSelected(!!value)}
      />
    ),
  }),
  columnHelper.accessor("fir_no", {
    header: "ایف آئی آر نمبر",
    meta: { label: "FIR no." },
    size: 110,
    cell: ({ row }) => (
      <div dir="ltr" className="font-medium font-mono text-right">
        {row.original.fir_no}
      </div>
    ),
  }),
  columnHelper.accessor("date", {
    header: "تاریخ ایف آئی آر",
    meta: { label: "FIR date" },
    size: 100,
    cell: ({ row }) => (
      <div dir="ltr" className="text-right">
        {formatDate(row.original.date)}
      </div>
    ),
  }),
  columnHelper.accessor("incident_date", {
    header: "تاریخ وقوعہ",
    meta: { label: "Incident date" },
    size: 100,
    cell: ({ row }) => (
      <div dir="ltr" className="text-right">
        {formatDate(row.original.incident_date)}
      </div>
    ),
  }),
  columnHelper.accessor("arrest_date", {
    header: "تاریخ گرفتاری",
    meta: { label: "Arrest date" },
    size: 100,
    cell: ({ row }) => (
      <div dir="ltr" className="text-right">
        {row.original.arrest_date ? formatDate(row.original.arrest_date) : "—"}
      </div>
    ),
  }),
  columnHelper.accessor("offence", {
    header: "جرم",
    meta: { label: "Offence" },
    size: 120,
    cell: ({ row }) => (
      <div dir="rtl" lang="ur" className="truncate text-right font-medium">
        {row.original.offence}
      </div>
    ),
  }),
  columnHelper.accessor("status", {
    header: "حالت",
    meta: { label: "Status" },
    size: 90,
    filterFn: "oneOf",
    cell: ({ row }) => <FirStatusBadge status={row.original.status} />,
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
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => event.stopPropagation()}
      >
        <FirRowActions fir={row.original} />
      </div>
    ),
  }),
]);

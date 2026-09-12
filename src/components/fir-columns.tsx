import { EditFirForm } from "./create-fir-form";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { Dialog, DialogContent, DialogTitle } from "./ui/dialog";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "./ui/alert-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { type DataTableFeatures } from "#/components/data-table-features";
import { formatDate } from "#/lib/date";
import { getFirStatusColor, getFirStatusLabel, type FirRecord } from "#/lib/fir";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { Exit } from "effect";
import { atoms } from "#/state/atoms";
import { cn } from "#/lib/utils";
import { Delete02Icon, Edit02Icon, MoreVerticalIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useAtomSet } from "@effect/atom-react";
import { createColumnHelper } from "@tanstack/react-table";
import { useState } from "react";
import { toast } from "#/components/ui/toast";

function FirRowActions({ fir }: { fir: FirRecord }) {
  const [isEditDialogOpen, setIsEditDialogOpen] = useState(false);
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
        <DropdownMenuTrigger render={<Button type="button" variant="ghost" size="icon-sm" />}>
          <HugeiconsIcon icon={MoreVerticalIcon} strokeWidth={2} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-36">
          <DropdownMenuItem onClick={() => setIsEditDialogOpen(true)}>
            <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
            Edit
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => setIsDeleteDialogOpen(true)} variant="destructive">
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <Dialog open={isEditDialogOpen} onOpenChange={setIsEditDialogOpen}>
        <DialogContent
          className="max-h-[calc(100dvh-1rem)] max-w-5xl overflow-y-auto p-4"
          showCloseButton={false}
        >
          <DialogTitle className="sr-only">ایف آئی آر میں ترمیم</DialogTitle>
          <EditFirForm
            className="max-w-none py-0"
            fir={fir}
            onSuccess={() => {
              setIsEditDialogOpen(false);
              toast.add({ title: "FIR updated", type: "success" });
            }}
          />
        </DialogContent>
      </Dialog>
      <AlertDialog open={isDeleteDialogOpen} onOpenChange={setIsDeleteDialogOpen}>
        <AlertDialogContent size="sm">
          <AlertDialogHeader>
            <AlertDialogTitle>Delete FIR</AlertDialogTitle>
            <AlertDialogDescription>
              This removes FIR {fir.fir_no} and its saved template placeholder values.
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
              Delete
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
    size: 28,
    header: ({ table }) => {
      const isAllSelected = table.getIsAllPageRowsSelected();
      const isSomeSelected = table.getIsSomePageRowsSelected();
      return (
        <Checkbox
          aria-label="تمام قطاریں منتخب کریں"
          checked={isAllSelected}
          indeterminate={isSomeSelected && !isAllSelected}
          onCheckedChange={(value) => table.toggleAllPageRowsSelected(!!value)}
        />
      );
    },
    cell: ({ row }) => (
      <Checkbox
        aria-label="قطار منتخب کریں"
        checked={row.getIsSelected()}
        onCheckedChange={(value) => row.toggleSelected(!!value)}
      />
    ),
  }),
  columnHelper.accessor("fir_no", {
    header: "ایف آئی آر ",
    size: 80,
    cell: ({ row }) => (
      <div dir="ltr" className="font-medium font-mono text-right">
        {row.original.fir_no}
      </div>
    ),
  }),
  columnHelper.accessor("date", {
    header: "تاریخ ایف آئی آر",
    size: 100,
    cell: ({ row }) => (
      <div dir="ltr" className="text-right">
        {formatDate(row.original.date)}
      </div>
    ),
  }),
  columnHelper.accessor("incident_date", {
    header: "تاریخ وقوعہ",
    size: 100,
    cell: ({ row }) => (
      <div dir="ltr" className="text-right">
        {formatDate(row.original.incident_date)}
      </div>
    ),
  }),
  columnHelper.accessor("arrest_date", {
    header: "تاریخ گرفتاری",
    size: 100,
    cell: ({ row }) => (
      <div dir="ltr" className="text-right">
        {row.original.arrest_date ? formatDate(row.original.arrest_date) : "—"}
      </div>
    ),
  }),
  columnHelper.accessor("offence", {
    header: "جرم",
    size: 70,
    cell: ({ row }) => (
      <div dir="rtl" lang="ur" className="truncate text-right font-medium">
        {row.original.offence}
      </div>
    ),
  }),
  columnHelper.accessor("status", {
    header: "اسٹیٹس",
    size: 70,
    filterFn: "equalsString",
    cell: ({ row }) => {
      const status = row.original.status;
      return (
        <Badge variant="outline">
          <span
            aria-hidden="true"
            className={cn("size-1.5 rounded-full", getFirStatusColor(status))}
          />
          {getFirStatusLabel(status)}
        </Badge>
      );
    },
  }),
  columnHelper.display({
    id: "actions",
    enableGlobalFilter: false,
    enableHiding: false,
    enableSorting: false,
    size: 50,
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

import type { ColumnDef } from "@tanstack/react-table";
import { useState } from "react";
import { toast } from "sonner";
import { Delete02Icon, Edit02Icon, MoreVerticalIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
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
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { CreateFirForm } from "./create-fir-form";
import { Dialog, DialogContent } from "./ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { firCollection, firPlaceholderValueCollection } from "#/db-collections";
import { cn } from "#/lib/utils";
import { getFirStatusColor, getFirStatusLabel } from "#/lib/fir";
import type { FirRecord } from "#/lib/fir";

function FirRowActions({ fir }: { fir: FirRecord }) {
  const [isEditDialogOpen, setIsEditDialogOpen] = useState(false);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);

  function handleDelete() {
    firCollection.delete(fir.id);

    for (const value of Array.from(firPlaceholderValueCollection.state.values())) {
      if (value.firId === fir.id) {
        firPlaceholderValueCollection.delete(value.id);
      }
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
        <DialogContent className="max-h-[calc(100dvh-1rem)] overflow-y-auto">
          <CreateFirForm
            fir={fir}
            onSuccess={() => {
              setIsEditDialogOpen(false);
              toast.success("FIR updated");
            }}
          />
        </DialogContent>
      </Dialog>
      <AlertDialog open={isDeleteDialogOpen} onOpenChange={setIsDeleteDialogOpen}>
        <AlertDialogContent size="sm">
          <AlertDialogHeader>
            <AlertDialogTitle>Delete FIR</AlertDialogTitle>
            <AlertDialogDescription>
              This removes FIR {fir.fir_no} and its saved template placeholder values from this
              browser.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={handleDelete} type="button" variant="destructive">
              <HugeiconsIcon data-icon="inline-start" icon={Delete02Icon} />
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

export const firColumns: ColumnDef<FirRecord>[] = [
  {
    cell: ({ row }) => (
      <Checkbox
        aria-label="قطار منتخب کریں"
        checked={row.getIsSelected()}
        onCheckedChange={(value) => row.toggleSelected(!!value)}
      />
    ),
    enableHiding: false,
    enableSorting: false,
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
    id: "select",
    size: 28,
  },
  {
    accessorKey: "fir_no",
    cell: ({ row }) => (
      <div dir="ltr" className="font-medium font-mono text-right">
        {row.original.fir_no}
      </div>
    ),
    header: "ایف آئی آر ",
    size: 80,
  },
  {
    accessorKey: "date",
    cell: ({ row }) => (
      <div dir="ltr" className="text-right">
        {row.original.date}
      </div>
    ),
    header: "تاریخ ایف آئی آر",
    size: 100,
  },
  {
    accessorKey: "incident_date",
    cell: ({ row }) => (
      <div dir="ltr" className="text-right">
        {row.original.incident_date}
      </div>
    ),
    header: "تاریخ وقوعہ",
    size: 100,
  },
  {
    accessorKey: "offence",
    cell: ({ row }) => (
      <div dir="rtl" lang="ur" className="truncate text-right font-medium">
        {row.original.offence}
      </div>
    ),
    header: "جرم",
    size: 70,
  },
  {
    accessorKey: "accused",
    cell: ({ row }) => (
      <div
        dir="rtl"
        lang="ur"
        className="line-clamp-1 min-w-0 text-right leading-6"
        title={row.original.accused}
      >
        {row.original.accused}
      </div>
    ),
    header: "ملزم",
    size: 190,
  },
  {
    accessorKey: "witness",
    cell: ({ row }) => (
      <div
        dir="rtl"
        lang="ur"
        className="line-clamp-1 min-w-0 text-right text-muted-foreground leading-6"
        title={row.original.witness || "—"}
      >
        {row.original.witness || "—"}
      </div>
    ),
    header: "گواہ",
    size: 180,
  },
  {
    accessorKey: "NIC",
    cell: ({ row }) => (
      <div dir="ltr" className="font-mono text-right">
        {row.original.NIC || "—"}
      </div>
    ),
    header: "شناختی کارڈ",
    size: 120,
  },
  {
    accessorKey: "mobile",
    cell: ({ row }) => (
      <div dir="ltr" className="font-mono text-right">
        {row.original.mobile || "—"}
      </div>
    ),
    header: "موبائل",
    size: 140,
  },
  {
    accessorKey: "status",
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
    header: "اسٹیٹس",
    size: 70,
  },
  {
    cell: ({ row }) => (
      <div
        data-no-row-click
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => event.stopPropagation()}
      >
        <FirRowActions fir={row.original} />
      </div>
    ),
    enableHiding: false,
    enableSorting: false,
    id: "actions",
    size: 50,
  },
];

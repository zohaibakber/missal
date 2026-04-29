import type { ColumnDef } from "@tanstack/react-table";
import {
  Delete02Icon,
  Edit02Icon,
  MoreVerticalIcon,
  Share08Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { cn } from "#/lib/utils";
import type { FirRecord } from "#/lib/fir";

const getStatusColor = (status: FirRecord["status"]) => {
  switch (status) {
    case "Open":
      return "bg-amber-500";
    case "Under Investigation":
      return "bg-blue-500";
    case "Challan Submitted":
      return "bg-violet-500";
    case "Closed":
      return "bg-emerald-500";
    default:
      return "bg-muted-foreground/64";
  }
};

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
            className={cn("size-1.5 rounded-full", getStatusColor(status))}
          />
          {status}
        </Badge>
      );
    },
    header: "اسٹیٹس",
    size: 70,
  },
  {
    cell: () => (
      <DropdownMenu>
        <DropdownMenuTrigger render={<Button type="button" variant="ghost" size="icon-sm" />}>
          <HugeiconsIcon icon={MoreVerticalIcon} strokeWidth={2} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-36">
          <DropdownMenuItem>
            <HugeiconsIcon icon={Share08Icon} strokeWidth={2} />
            Share
          </DropdownMenuItem>
          <DropdownMenuItem>
            <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
            Edit
          </DropdownMenuItem>
          <DropdownMenuItem variant="destructive">
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    ),
    enableHiding: false,
    enableSorting: false,
    id: "actions",
    size: 50,
  },
];

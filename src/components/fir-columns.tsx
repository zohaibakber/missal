import type { ColumnDef } from "@tanstack/react-table";
import { Checkbox } from "./ui/checkbox";
import type { Flight } from "#/lib/fir-data";
import { Badge } from "./ui/badge";
import { HugeiconsIcon } from "@hugeicons/react";
import { PlaneTakeoff } from "@hugeicons/core-free-icons";
import { cn } from "#/lib/utils";

const getStatusColor = (status: Flight["status"]) => {
  switch (status) {
    case "On Time":
      return "bg-emerald-500";
    case "Delayed":
      return "bg-amber-500";
    case "Cancelled":
      return "bg-red-500";
    case "Boarding":
      return "bg-blue-500";
    default:
      return "bg-muted-foreground/64";
  }
};

export const firColumns: ColumnDef<Flight>[] = [
  {
    cell: ({ row }) => (
      <Checkbox
        aria-label="Select row"
        checked={row.getIsSelected()}
        onCheckedChange={(value) => row.toggleSelected(!!value)}
      />
    ),
    enableSorting: false,
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
    id: "select",
    size: 28,
  },
  {
    accessorKey: "flightCode",
    cell: ({ row }) => (
      <div className="font-medium font-mono text-muted-foreground">
        {row.getValue("flightCode")}
      </div>
    ),
    header: "Flight",
    size: 80,
  },
  {
    accessorKey: "departureTime",
    cell: ({ row }) => {
      const isCancelled = row.original.status === "Cancelled";
      const isDelayed = row.original.status === "Delayed";
      return (
        <div
          className={cn(
            "flex items-center gap-1.5 font-normal tabular-nums",
            isCancelled && "text-muted-foreground line-through opacity-50",
          )}
        >
          <div className={isDelayed ? "text-warning-foreground" : undefined}>
            {row.original.departureTime}
          </div>
          <div
            aria-hidden="true"
            className="flex items-center gap-0.5 opacity-50 before:size-1.5 before:rounded-full before:border before:border-muted-foreground after:h-px after:w-3 after:border-muted-foreground after:border-t after:border-dashed"
          />
          <div className={cn("text-muted-foreground", isCancelled && "line-through")}>
            {row.original.duration}
          </div>
          <div
            aria-hidden="true"
            className="flex items-center gap-0.5 opacity-50 before:order-1 before:size-1.5 before:rounded-full before:border before:border-muted-foreground after:h-px after:w-3 after:border-muted-foreground after:border-t after:border-dashed"
          />
          <div>{row.original.arrivalTime}</div>
        </div>
      );
    },
    header: "Time",
    size: 220,
  },
  {
    accessorKey: "destination",
    cell: ({ row }) => <div className="font-medium">{row.getValue("destination")}</div>,
    header: "Destination",
    size: 180,
  },
  {
    accessorKey: "status",
    cell: ({ row }) => {
      const status = row.getValue("status") as Flight["status"];
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
    header: "Status",
    size: 120,
  },
  {
    accessorKey: "terminal",
    cell: ({ row }) => (
      <Badge className="font-normal tabular-nums" variant="outline">
        <HugeiconsIcon icon={PlaneTakeoff} strokeWidth={2} data-icon="inline-start" />
        <span>{row.getValue("terminal")}</span>
      </Badge>
    ),
    header: "Terminal",
    size: 90,
  },
  {
    accessorKey: "gate",
    header: "Gate",
    size: 80,
  },
];

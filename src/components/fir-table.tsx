import type { SortingState } from "@tanstack/react-table";
import { useLiveQuery } from "@tanstack/react-db";
import { useNavigate } from "@tanstack/react-router";
import type { DataTableToolbarConfig } from "#/components/data-table-toolbar";
import { DataTable } from "./data-table";
import { firColumns } from "./fir-columns";
import { firCollection } from "#/db-collections";
import type { FirRecord } from "#/lib/fir";
import { FIR_STATUS_OPTIONS, getFirStatusLabel } from "#/lib/fir";
import { Card, CardFooter } from "#/components/ui/card";
import { Skeleton } from "#/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#/components/ui/table";

const firTableToolbar: DataTableToolbarConfig<FirRecord> = {
  search: {
    label: "Search FIR records",
    searchableColumnIds: [
      "fir_no",
      "date",
      "incident_date",
      "arrest_date",
      "offence",
      "accused",
      "witness",
      "NIC",
      "mobile",
      "investigation_officer",
      "status",
    ],
  },
  filters: [
    {
      columnId: "status",
      label: "حالت",
      placeholder: "تمام حالتیں",
      options: FIR_STATUS_OPTIONS.map((status) => ({
        label: getFirStatusLabel(status),
        value: status,
      })),
    },
  ],
};

const initialSorting: SortingState = [
  {
    desc: true,
    id: "fir_no",
  },
];

export function FirTable() {
  const { data } = useLiveQuery(firCollection);
  const navigate = useNavigate();

  return (
    <DataTable
      columns={firColumns}
      data={data}
      initialSorting={initialSorting}
      onRowClick={(fir) => {
        void navigate({
          to: "/$firId",
          params: { firId: `${fir.id}` },
        });
      }}
      tableDir="rtl"
      tableLang="ur"
      toolbar={firTableToolbar}
    />
  );
}

const skeletonRows = Array.from({ length: 10 }, (_, index) => index);

export function FirTableSkeleton() {
  return (
    <Card className="w-full gap-0 py-0" aria-busy="true">
      <div className="flex flex-col gap-3 border-b bg-muted p-2 py-2.5" dir="rtl">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <Skeleton className="h-8 w-full bg-background lg:max-w-64" />
          <div className="flex flex-1 items-center justify-end gap-2">
            <Skeleton className="size-7 bg-background" />
            <Skeleton className="size-7 bg-background" />
          </div>
        </div>
      </div>

      <div dir="rtl" lang="ur">
        <Table className="table-fixed">
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              {firColumns.map((column, index) => (
                <TableHead
                  key={column.id ?? `column-${index}`}
                  style={column.size ? { width: `${column.size}px` } : undefined}
                >
                  <Skeleton className="h-4 w-full" />
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {skeletonRows.map((row) => (
              <TableRow key={row}>
                {firColumns.map((column, index) => (
                  <TableCell key={column.id ?? `cell-${row}-${index}`}>
                    <Skeleton className="h-5 w-full" />
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>

      <CardFooter className="p-2">
        <div className="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-center gap-2">
            <Skeleton className="h-5 w-14" />
            <Skeleton className="h-8 w-20" />
            <Skeleton className="h-5 w-24" />
          </div>
          <div className="flex items-center gap-2">
            <Skeleton className="size-8" />
            <Skeleton className="size-8" />
            <Skeleton className="size-8" />
            <Skeleton className="size-8" />
          </div>
        </div>
      </CardFooter>
    </Card>
  );
}

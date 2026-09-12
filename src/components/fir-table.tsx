import type { SortingState } from "@tanstack/react-table";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Link, useNavigate } from "@tanstack/react-router";
import type { DataTableToolbarConfig } from "#/components/data-table-toolbar";
import { DataTable } from "./data-table";
import { firColumns } from "./fir-columns";
import type { FirRecord } from "#/lib/fir";
import { FIR_STATUS_OPTIONS, getFirStatusLabel } from "#/lib/fir";
import { atoms } from "#/state/atoms";
import { Button } from "#/components/ui/button";
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
  action: (
    <Button nativeButton={false} render={<Link to="/new" />}>
      ایف آئی آر درج کریں
    </Button>
  ),
  search: {
    label: "Search FIR records",
    searchableColumnIds: ["fir_no", "date", "incident_date", "arrest_date", "offence", "status"],
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
  const firs = useAtomValue(atoms.firsAtom);
  const navigate = useNavigate();

  if (AsyncResult.isInitial(firs) || AsyncResult.isWaiting(firs)) {
    return <FirTableSkeleton />;
  }

  const data = AsyncResult.isSuccess(firs) ? firs.value : [];

  return (
    <DataTable
      columns={firColumns}
      data={[...data]}
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
    <div className="flex w-full flex-col" aria-busy="true">
      <div className="flex flex-col gap-3 py-4" dir="rtl">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <Skeleton className="h-8 w-full lg:max-w-64" />
          <div className="flex flex-1 items-center justify-end gap-2">
            <Skeleton className="size-7" />
            <Skeleton className="size-7" />
          </div>
        </div>
      </div>

      <div className="overflow-hidden rounded-md border" dir="rtl" lang="ur">
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

      <div className="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
          <Skeleton className="h-5 w-40" />
          <div className="flex items-center gap-2">
            <Skeleton className="h-5 w-14" />
            <Skeleton className="h-8 w-20" />
            <Skeleton className="h-5 w-24" />
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Skeleton className="size-8" />
          <Skeleton className="size-8" />
          <Skeleton className="size-8" />
          <Skeleton className="size-8" />
        </div>
      </div>
    </div>
  );
}

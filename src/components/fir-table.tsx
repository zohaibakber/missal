import type { SortingState } from "@tanstack/react-table";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Link, useNavigate } from "@tanstack/react-router";
import { Add01Icon, LegalDocument01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { DataTable, DataTablePagination, useDataTable } from "#/components/data-table";
import {
  DataTableFacetedFilter,
  DataTableResetFilters,
  DataTableSearch,
  DataTableToolbar,
  DataTableViewOptions,
} from "#/components/data-table-toolbar";
import { firColumns } from "#/components/fir-columns";
import { FirStatusDot } from "#/components/fir-status-badge";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Button } from "#/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import { Skeleton } from "#/components/ui/skeleton";
import { FIR_STATUS_OPTIONS, getFirStatusLabel, type FirRecord } from "#/lib/fir";
import { atoms } from "#/state/atoms";

const initialSorting: SortingState = [{ desc: true, id: "fir_no" }];

const statusOptions = FIR_STATUS_OPTIONS.map((status) => ({
  value: status,
  label: (
    <span lang="ur" dir="rtl">
      {getFirStatusLabel(status)}
    </span>
  ),
  icon: <FirStatusDot status={status} />,
}));

export function NewFirButton() {
  return (
    <Button nativeButton={false} render={<Link to="/new" />}>
      <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
      New FIR
      <ShortcutKbd id="newFir" />
    </Button>
  );
}

export function FirTable() {
  const firs = useAtomValue(atoms.firsAtom);

  if (AsyncResult.isSuccess(firs)) {
    return firs.value.length ? <FirDataTable data={[...firs.value]} /> : <NoFirs />;
  }

  if (AsyncResult.isFailure(firs)) {
    return (
      <Empty variant="outline">
        <EmptyHeader>
          <EmptyTitle>Could not load FIRs</EmptyTitle>
          <EmptyDescription>
            The database could not be read. Restart Missal and try again.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return <FirTableSkeleton />;
}

function FirDataTable({ data }: { data: FirRecord[] }) {
  const navigate = useNavigate();
  const table = useDataTable({ columns: firColumns, data, initialSorting });

  return (
    <div className="flex flex-col gap-4">
      <DataTableToolbar>
        <DataTableSearch table={table} placeholder="Search FIRs…" />
        <DataTableFacetedFilter
          table={table}
          columnId="status"
          title="Status"
          options={statusOptions}
        />
        <DataTableResetFilters table={table} />
        <div className="ms-auto">
          <DataTableViewOptions table={table} />
        </div>
      </DataTableToolbar>
      <DataTable
        table={table}
        dir="rtl"
        lang="ur"
        onRowActivate={(fir) => void navigate({ to: "/$firId", params: { firId: `${fir.id}` } })}
      />
      <DataTablePagination table={table} />
    </div>
  );
}

function NoFirs() {
  return (
    <Empty variant="outline" className="min-h-96">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <HugeiconsIcon icon={LegalDocument01Icon} />
        </EmptyMedia>
        <EmptyTitle>No FIRs yet</EmptyTitle>
        <EmptyDescription>Create an FIR to start writing its documents.</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <NewFirButton />
      </EmptyContent>
    </Empty>
  );
}

const skeletonRows = Array.from({ length: 8 }, (_, index) => index);

function FirTableSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true">
      <div className="flex items-center gap-2">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-8 w-24" />
        <Skeleton className="ms-auto h-8 w-20" />
      </div>
      <div className="flex flex-col gap-2">
        {skeletonRows.map((row) => (
          <Skeleton key={row} className="h-11" />
        ))}
      </div>
    </div>
  );
}

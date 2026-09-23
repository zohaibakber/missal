import { useState } from "react";
import type { SortingState } from "@tanstack/react-table";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit } from "effect";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  Add01Icon,
  ArrowUpRight01Icon,
  Delete02Icon,
  Edit02Icon,
  Folder01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { DataTable, useDataTable } from "#/components/data-table";
import {
  DataTableFacetedFilter,
  DataTableResetFilters,
  DataTableSearch,
  DataTableViewOptions,
} from "#/components/data-table-toolbar";
import { EditFirSheet } from "#/components/edit-fir-sheet";
import { firColumns, FirRowActionsContext, type FirRowActions } from "#/components/fir-columns";
import { FirStatusDot } from "#/components/fir-status-badge";
import { Hint } from "#/components/hint";
import {
  Pane,
  PaneActions,
  PaneBody,
  PaneHeader,
  PaneStatusBar,
  PaneTitle,
} from "#/components/pane";
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
import { Button } from "#/components/ui/button";
import {
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuSeparator,
} from "#/components/ui/context-menu";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import { Skeleton } from "#/components/ui/skeleton";
import { Spinner } from "#/components/ui/spinner";
import { toast } from "#/components/ui/toast";
import { FIR_STATUS_OPTIONS, getFirStatusLabel, type FirRecord } from "#/lib/fir";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
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

function NewFirButton() {
  return (
    <Hint label="New FIR" shortcut="newFir">
      <Button size="sm" className="ms-1" nativeButton={false} render={<Link to="/new" />}>
        <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
        New FIR
      </Button>
    </Hint>
  );
}

/** The FIR list: toolbar, sortable/filterable table and a status bar. */
export function FirsPane() {
  const firs = useAtomValue(atoms.firsAtom);

  if (AsyncResult.isSuccess(firs) && firs.value.length) {
    return <FirBrowser data={firs.value} />;
  }

  return (
    <Pane>
      <PaneHeader>
        <PaneTitle>FIRs</PaneTitle>
        <PaneActions>
          <NewFirButton />
        </PaneActions>
      </PaneHeader>
      <PaneBody className="flex">
        {AsyncResult.isSuccess(firs) ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={Folder01Icon} strokeWidth={2} />
              </EmptyMedia>
              <EmptyTitle>No FIRs yet</EmptyTitle>
              <EmptyDescription>
                Create an FIR, then add templates to write its documents.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : AsyncResult.isFailure(firs) ? (
          <Empty>
            <EmptyHeader>
              <EmptyTitle>Could not load FIRs</EmptyTitle>
              <EmptyDescription>The database could not be read. Restart Missal.</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <FirTableSkeleton />
        )}
      </PaneBody>
    </Pane>
  );
}

function FirBrowser({ data }: { data: readonly FirRecord[] }) {
  const navigate = useNavigate();
  const table = useDataTable({ columns: firColumns, data: [...data], initialSorting });
  // Targets outlive their `open` flags so the sheet and dialog keep their content while closing.
  const [editing, setEditing] = useState<FirRecord | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [deleting, setDeleting] = useState<readonly FirRecord[]>([]);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const selected = table.getSelectedRowModel().rows.map((row) => row.original);
  const visibleCount = table.getRowModel().rows.length;
  const actions: FirRowActions = {
    edit: (fir) => {
      setEditing(fir);
      setEditOpen(true);
    },
    remove: (firs) => {
      setDeleting(firs);
      setDeleteOpen(true);
    },
  };
  const open = (fir: FirRecord) => void navigate({ to: "/$firId", params: { firId: `${fir.id}` } });

  return (
    <FirRowActionsContext value={actions}>
      <Pane>
        <PaneHeader>
          <PaneTitle>FIRs</PaneTitle>
          <div className="ms-3 flex min-w-0 items-center gap-1">
            <DataTableSearch table={table} placeholder="Search FIRs" />
            <DataTableFacetedFilter
              table={table}
              columnId="status"
              title="Status"
              options={statusOptions}
            />
            <DataTableResetFilters table={table} />
          </div>
          <PaneActions>
            <DataTableViewOptions table={table} />
            <NewFirButton />
          </PaneActions>
        </PaneHeader>
        <DataTable
          table={table}
          dir="rtl"
          lang="ur"
          onRowActivate={open}
          rowContextMenu={(fir) => (
            <div dir="ltr" lang="en">
              <ContextMenuGroup>
                <ContextMenuItem onClick={() => open(fir)}>
                  <HugeiconsIcon icon={ArrowUpRight01Icon} strokeWidth={2} />
                  Open
                </ContextMenuItem>
                <ContextMenuItem onClick={() => actions.edit(fir)}>
                  <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
                  Edit details
                </ContextMenuItem>
              </ContextMenuGroup>
              <ContextMenuSeparator />
              <ContextMenuGroup>
                <ContextMenuItem variant="destructive" onClick={() => actions.remove([fir])}>
                  <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                  Delete
                </ContextMenuItem>
              </ContextMenuGroup>
            </div>
          )}
          empty={<p className="text-center text-muted-foreground">No FIRs match.</p>}
        />
        <PaneStatusBar>
          {selected.length ? (
            <>
              <span className="text-foreground">{selected.length} selected</span>
              <Button variant="subtle" size="xs" onClick={() => table.resetRowSelection()}>
                Clear
              </Button>
              <Button
                variant="destructive"
                size="xs"
                className="ms-auto"
                onClick={() => actions.remove(selected)}
              >
                Delete {selected.length === 1 ? "FIR" : `${selected.length} FIRs`}
              </Button>
            </>
          ) : visibleCount === data.length ? (
            `${data.length} ${data.length === 1 ? "FIR" : "FIRs"}`
          ) : (
            `${visibleCount} of ${data.length} FIRs`
          )}
        </PaneStatusBar>
      </Pane>
      {editing ? <EditFirSheet fir={editing} open={editOpen} onOpenChange={setEditOpen} /> : null}
      <DeleteFirsDialog
        firs={deleting}
        open={deleteOpen}
        onClose={() => setDeleteOpen(false)}
        onDeleted={() => table.resetRowSelection()}
      />
    </FirRowActionsContext>
  );
}

function DeleteFirsDialog({
  firs,
  open,
  onClose,
  onDeleted,
}: {
  firs: readonly FirRecord[];
  open: boolean;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const removeFir = useAtomSet(atoms.removeFirAtom, { mode: "promiseExit" });
  const [pending, setPending] = useState(false);
  const [first] = firs;

  async function handleDelete() {
    setPending(true);
    try {
      for (const fir of firs) {
        const exit = await removeFir(fir.id);
        if (Exit.isFailure(exit)) {
          toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
          return;
        }
      }
      onDeleted();
      onClose();
    } finally {
      setPending(false);
    }
  }

  return (
    <AlertDialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {firs.length === 1 && first
              ? `Delete FIR ${first.fir_no}?`
              : `Delete ${firs.length} FIRs?`}
          </AlertDialogTitle>
          <AlertDialogDescription>
            Every document written for {firs.length === 1 ? "this FIR" : "these FIRs"} is deleted
            too. This can't be undone.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            disabled={pending}
            onClick={(event) => {
              event.preventDefault();
              void handleDelete();
            }}
            type="button"
            variant="destructive"
          >
            {pending ? <Spinner data-icon="inline-start" /> : null}
            Delete
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

const skeletonRows = Array.from({ length: 8 }, (_, index) => index);

function FirTableSkeleton() {
  return (
    <div className="flex w-full flex-col" aria-busy="true">
      <div className="h-9 border-b" />
      {skeletonRows.map((row) => (
        <div key={row} className="flex h-10 items-center gap-6 border-b px-4">
          <Skeleton className="h-3 w-24" />
          <Skeleton className="h-3 w-32" />
          <Skeleton className="ms-auto h-3 w-20" />
        </div>
      ))}
    </div>
  );
}

import { useEffect, useMemo, useRef, useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Cause, Exit, Match, Option } from "effect";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  Add01Icon,
  ArrowDown01Icon,
  ArrowUp01Icon,
  ArrowUpRight01Icon,
  Delete02Icon,
  Edit02Icon,
  LegalDocument01Icon,
  MoreHorizontalIcon,
  PrinterIcon,
  TextFontIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { AddTemplatesPopover } from "#/components/add-templates-popover";
import { DocumentEditor } from "#/components/document-editor";
import { EditFirSheet } from "#/components/edit-fir-sheet";
import { FirStatusBadge } from "#/components/fir-status-badge";
import { Hint } from "#/components/hint";
import { IconAction } from "#/components/icon-action";
import {
  Pane,
  PaneActions,
  PaneBody,
  PaneHeader,
  PaneStatusBar,
  PaneTitle,
  SaveStatus,
} from "#/components/pane";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import {
  SplitView,
  SplitViewDetail,
  SplitViewList,
  SplitViewListHeader,
  SplitViewListTitle,
} from "#/components/split-view";
import { UnsavedChanges } from "#/components/unsaved-changes";
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
  ContextMenu,
  ContextMenuCheckboxItem,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "#/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSwitchItem,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import {
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuCheckbox,
  SidebarMenuItem,
} from "#/components/ui/sidebar";
import { Skeleton } from "#/components/ui/skeleton";
import { Spinner } from "#/components/ui/spinner";
import { toast } from "#/components/ui/toast";
import { printEnvelopePacket } from "#/editor/html-export";
import type { EditorSessionHandle } from "#/editor/session";
import { useShortcut } from "#/hooks/use-shortcut";
import { catalogFieldPresentation, type FieldDisplayMode } from "#/lib/field";
import {
  FirDocumentSaveInput,
  ReorderFirDocumentsInput,
  type FirDocumentSummary,
} from "#/lib/fir-document";
import { parseFirDocumentId, type FirDocumentId, type FirId } from "#/lib/ids";
import { indexPlaceholders } from "#/lib/placeholder";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { atoms } from "#/state/atoms";

const EMPTY_DOCUMENTS: readonly FirDocumentSummary[] = [];

type FirWorkspaceProps = {
  documentId?: string;
  firId: FirId;
};

export function FirWorkspace({ documentId: documentIdParam, firId }: FirWorkspaceProps) {
  const navigate = useNavigate();
  const firResult = useAtomValue(atoms.firByIdAtom(firId));
  const documentsResult = useAtomValue(atoms.firDocumentsAtom(firId));
  const valueContextResult = useAtomValue(atoms.firValueContextAtom(firId));
  const placeholderIndexResult = useAtomValue(atoms.placeholderIndexAtom);
  const removeFir = useAtomSet(atoms.removeFirAtom, { mode: "promiseExit" });
  const loadPrintDocuments = useAtomSet(atoms.loadPrintDocumentsAtom, { mode: "promiseExit" });
  const saveDocument = useAtomSet(atoms.saveFirDocumentAtom, { mode: "promiseExit" });
  const reorderDocuments = useAtomSet(atoms.reorderFirDocumentsAtom, { mode: "promiseExit" });
  const removeDocument = useAtomSet(atoms.removeFirDocumentAtom, { mode: "promiseExit" });
  const documents = AsyncResult.isSuccess(documentsResult)
    ? documentsResult.value
    : EMPTY_DOCUMENTS;
  const placeholderIndex = AsyncResult.isSuccess(placeholderIndexResult)
    ? placeholderIndexResult.value
    : indexPlaceholders([]);
  const valueContext = AsyncResult.isSuccess(valueContextResult) ? valueContextResult.value : null;
  const requestedDocumentId = documentIdParam ? parseFirDocumentId(documentIdParam) : undefined;
  const activeId =
    (requestedDocumentId && documents.some((document) => document.id === requestedDocumentId)
      ? requestedDocumentId
      : documents[0]?.id) ?? null;
  const activeDocumentResult = useAtomValue(atoms.firDocumentByIdAtom(activeId));
  const activeDocument =
    activeId && AsyncResult.isSuccess(activeDocumentResult) ? activeDocumentResult.value : null;
  const [addTemplatesOpen, setAddTemplatesOpen] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [savePending, setSavePending] = useState(false);
  const [displayMode, setDisplayMode] = useState<FieldDisplayMode>("values");
  const [outputIds, setOutputIds] = useState<ReadonlySet<FirDocumentId>>(() => new Set());
  const [isDeleteFirOpen, setIsDeleteFirOpen] = useState(false);
  const [pendingId, setPendingId] = useState<FirDocumentId | null>(null);
  const [revision, setRevision] = useState(activeDocument?.revision);
  const [editorEpoch, setEditorEpoch] = useState(0);
  const sessionRef = useRef<EditorSessionHandle | null>(null);
  const loadedIdRef = useRef<FirDocumentId | null>(null);
  const previousDocumentIds = useRef<ReadonlySet<FirDocumentId>>(new Set());

  const presentation = useMemo(
    () =>
      valueContext
        ? valueContext.toPresentation(displayMode)
        : catalogFieldPresentation(placeholderIndex, displayMode),
    [displayMode, placeholderIndex, valueContext],
  );

  useEffect(() => {
    if (!activeDocument) {
      return;
    }

    if (loadedIdRef.current !== activeDocument.id) {
      loadedIdRef.current = activeDocument.id;
      setRevision(activeDocument.revision);
      setDirty(false);
    }
  }, [activeDocument]);

  useEffect(() => {
    const previous = previousDocumentIds.current;
    const nextIds = new Set(documents.map((document) => document.id));
    setOutputIds(
      (current) => new Set([...nextIds].filter((id) => current.has(id) || !previous.has(id))),
    );
    previousDocumentIds.current = nextIds;
  }, [documents]);

  if (AsyncResult.isInitial(firResult) || AsyncResult.isInitial(documentsResult)) {
    return <FirWorkspaceSkeleton />;
  }

  if (AsyncResult.isFailure(firResult)) {
    return Option.match(Cause.findErrorOption(firResult.cause), {
      onNone: () => <FirWorkspaceError />,
      onSome: (error) =>
        Match.value(error).pipe(
          Match.tag("EntityNotFound", () => <FirNotFound />),
          Match.orElse(() => <FirWorkspaceError />),
        ),
    });
  }

  if (AsyncResult.isFailure(documentsResult)) return <FirWorkspaceError />;

  const fir = firResult.value;

  function selectDocument(id: FirDocumentId) {
    if (id === activeId) {
      return;
    }

    if (dirty) {
      setPendingId(id);
      return;
    }

    void navigate({
      params: { firId: `${firId}` },
      replace: true,
      search: { documentId: `${id}` },
      to: "/$firId",
    });
  }

  async function persistActive() {
    const session = sessionRef.current;
    if (!activeDocument || !session || revision === undefined) {
      return false;
    }

    if (!session.tryBeginSave()) {
      return false;
    }

    const captured = session.captureEnvelope();
    try {
      const exit = await saveDocument(
        new FirDocumentSaveInput({
          document: captured.envelope,
          expectedRevision: revision,
          id: activeDocument.id,
        }),
      );

      if (Exit.isFailure(exit)) {
        toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
        return false;
      }

      session.markSaved(captured.contentRevision);
      setRevision(exit.value.revision);
      return true;
    } finally {
      session.endSave();
    }
  }

  async function handleSave() {
    const saved = await persistActive();
    if (saved) {
      toast.add({ title: "Document saved", type: "success" });
    }
  }

  async function confirmPending(action: "save" | "discard") {
    const nextId = pendingId;
    if (action === "save") {
      const saved = await persistActive();
      if (!saved) {
        return;
      }
    } else {
      loadedIdRef.current = null;
      setEditorEpoch((epoch) => epoch + 1);
      setDirty(false);
    }

    setPendingId(null);
    if (nextId) {
      void navigate({
        params: { firId: `${firId}` },
        replace: true,
        search: { documentId: `${nextId}` },
        to: "/$firId",
      });
    }
  }

  async function handleReorder(fromIndex: number, toIndex: number) {
    if (toIndex < 0 || toIndex >= documents.length) {
      return;
    }

    const documentIds = documents.map((document) => document.id);
    const [moved] = documentIds.splice(fromIndex, 1);
    if (!moved) {
      return;
    }
    documentIds.splice(toIndex, 0, moved);
    const exit = await reorderDocuments(new ReorderFirDocumentsInput({ documentIds, firId }));
    if (Exit.isFailure(exit)) {
      toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
    }
  }

  async function handleRemoveDocument(id: FirDocumentId) {
    const exit = await removeDocument(id);
    if (Exit.isFailure(exit)) {
      toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
      return;
    }

    if (id === activeId) {
      loadedIdRef.current = null;
      setDirty(false);
    }
  }

  async function handleDeleteFir() {
    const exit = await removeFir(firId);
    if (Exit.isFailure(exit)) {
      toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
      return;
    }

    setIsDeleteFirOpen(false);
    void navigate({ to: "/" });
  }

  async function handlePrint() {
    const selected = documents.filter((document) => outputIds.has(document.id));
    if (!selected.length) {
      toast.add({ title: "Select at least one document to print", type: "warning" });
      return;
    }

    const captured =
      activeDocument && sessionRef.current ? sessionRef.current.captureEnvelope() : null;
    const result = await loadPrintDocuments(selected.map((document) => document.id));
    if (Exit.isFailure(result)) {
      toast.add({ title: getRepositoryErrorMessage(result), type: "error" });
      return;
    }
    const title = `FIR ${fir.fir_no}`;
    if (
      !printEnvelopePacket(
        result.value.map((document) => ({
          _tag: "Envelope",
          envelope:
            document.id === activeDocument?.id && captured ? captured.envelope : document.document,
          presentation,
        })),
        title,
      )
    ) {
      toast.add({ title: "Unable to prepare print view", type: "error" });
    }
  }

  function stepDocument(offset: 1 | -1) {
    const index = documents.findIndex((document) => document.id === activeId);
    const next = documents[index + offset];
    if (next) selectDocument(next.id);
  }

  const printCount = outputIds.size;

  return (
    <Pane>
      <WorkspaceShortcuts
        canSave={dirty && !savePending}
        onSave={() => void handleSave()}
        onPrint={() => void handlePrint()}
        onEditDetails={() => setDetailsOpen(true)}
        onStepDocument={stepDocument}
        onToggleFieldNames={() =>
          setDisplayMode((mode) => (mode === "labels" ? "values" : "labels"))
        }
      />
      <UnsavedChanges isDirty={() => dirty && pendingId === null} />
      <PaneHeader>
        <PaneTitle className="flex-none">FIR {fir.fir_no}</PaneTitle>
        <FirStatusBadge status={fir.status} />
        <span
          lang="ur"
          dir="rtl"
          className="hidden min-w-0 truncate text-ur text-muted-foreground lg:block"
        >
          {fir.offence}
        </span>
        <PaneActions>
          {activeDocument ? <SaveStatus dirty={dirty} /> : null}
          <IconAction
            label="Edit FIR details"
            shortcut="editFir"
            onClick={() => setDetailsOpen(true)}
          >
            <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
          </IconAction>
          <IconAction
            label={
              printCount
                ? `Print ${printCount} ${printCount === 1 ? "document" : "documents"}`
                : "Print"
            }
            shortcut="print"
            disabled={!printCount}
            onClick={() => void handlePrint()}
          >
            <HugeiconsIcon icon={PrinterIcon} strokeWidth={2} />
          </IconAction>
          <DropdownMenu>
            <Hint label="More actions">
              <DropdownMenuTrigger
                render={
                  <Button size="icon-sm" type="button" variant="ghost" aria-label="More actions" />
                }
              >
                <HugeiconsIcon icon={MoreHorizontalIcon} strokeWidth={2} />
              </DropdownMenuTrigger>
            </Hint>
            <DropdownMenuContent align="end" className="w-64">
              <DropdownMenuGroup>
                <DropdownMenuSwitchItem
                  checked={displayMode === "labels"}
                  onCheckedChange={(checked) => setDisplayMode(checked ? "labels" : "values")}
                >
                  <HugeiconsIcon icon={TextFontIcon} strokeWidth={2} />
                  Show field names
                  <DropdownMenuShortcut>
                    <ShortcutKbd id="toggleFieldNames" />
                  </DropdownMenuShortcut>
                </DropdownMenuSwitchItem>
                <DropdownMenuItem onClick={() => setDetailsOpen(true)}>
                  <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
                  Edit FIR details
                  <DropdownMenuShortcut>
                    <ShortcutKbd id="editFir" />
                  </DropdownMenuShortcut>
                </DropdownMenuItem>
              </DropdownMenuGroup>
              <DropdownMenuSeparator />
              <DropdownMenuGroup>
                <DropdownMenuItem onClick={() => setIsDeleteFirOpen(true)} variant="destructive">
                  <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                  Delete FIR
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>
          <Hint label="Save document" shortcut="save">
            <Button
              className="ms-1"
              size="sm"
              disabled={!dirty || savePending}
              onClick={() => void handleSave()}
              type="button"
            >
              {savePending ? <Spinner data-icon="inline-start" /> : null}
              Save
            </Button>
          </Hint>
        </PaneActions>
      </PaneHeader>

      <SplitView id="fir-workspace">
        <SplitViewList>
          <SplitViewListHeader className="h-10">
            <SplitViewListTitle className="text-xs text-muted-foreground">
              Documents
              {documents.length ? (
                <span className="font-normal tabular-nums">{documents.length}</span>
              ) : null}
            </SplitViewListTitle>
            <AddTemplatesPopover
              firId={firId}
              open={addTemplatesOpen}
              onOpenChange={setAddTemplatesOpen}
              onAdded={selectDocument}
            />
          </SplitViewListHeader>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {documents.length ? (
              <SidebarMenu className="gap-px p-2" aria-label="Documents">
                {documents.map((document, index) => (
                  <DocumentRow
                    key={document.id}
                    title={document.title}
                    active={document.id === activeId}
                    printed={outputIds.has(document.id)}
                    canMoveUp={index > 0}
                    canMoveDown={index < documents.length - 1}
                    canRemove={!(dirty && document.id === activeId)}
                    onSelect={() => selectDocument(document.id)}
                    onPrintedChange={(checked) =>
                      setOutputIds((current) => {
                        const next = new Set(current);
                        if (checked) next.add(document.id);
                        else next.delete(document.id);
                        return next;
                      })
                    }
                    onMove={(offset) => void handleReorder(index, index + offset)}
                    onRemove={() => void handleRemoveDocument(document.id)}
                  />
                ))}
              </SidebarMenu>
            ) : (
              <Empty size="sm">
                <EmptyHeader>
                  <EmptyTitle>No documents</EmptyTitle>
                  <EmptyDescription>
                    Press <ShortcutKbd id="addTemplates" /> to add templates.
                  </EmptyDescription>
                </EmptyHeader>
              </Empty>
            )}
          </div>
          {documents.length ? (
            <PaneStatusBar className="px-3">
              {printCount === documents.length
                ? "All documents will print"
                : `${printCount} of ${documents.length} will print`}
            </PaneStatusBar>
          ) : null}
        </SplitViewList>
        <SplitViewDetail>
          {activeDocument && revision !== undefined ? (
            <DocumentEditor
              aria-label={activeDocument.title}
              document={activeDocument.document}
              onDirtyChange={setDirty}
              onSavePendingChange={setSavePending}
              onSessionReady={(session) => {
                sessionRef.current = session;
              }}
              placeholderIndex={presentation.catalog}
              presentation={presentation}
              sessionKey={`${activeDocument.id}:${editorEpoch}`}
            />
          ) : documents.length ? (
            <EditorSkeleton />
          ) : (
            <div className="flex h-full bg-canvas">
              <Empty>
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <HugeiconsIcon icon={LegalDocument01Icon} strokeWidth={2} />
                  </EmptyMedia>
                  <EmptyTitle>Start with a template</EmptyTitle>
                  <EmptyDescription>
                    Each template becomes a document filled with this FIR's details.
                  </EmptyDescription>
                </EmptyHeader>
                <EmptyContent>
                  <Hint label="Add templates" shortcut="addTemplates">
                    <Button variant="outline" size="sm" onClick={() => setAddTemplatesOpen(true)}>
                      <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
                      Add templates
                    </Button>
                  </Hint>
                </EmptyContent>
              </Empty>
            </div>
          )}
        </SplitViewDetail>
      </SplitView>

      <EditFirSheet fir={fir} open={detailsOpen} onOpenChange={setDetailsOpen} />

      <AlertDialog onOpenChange={setIsDeleteFirOpen} open={isDeleteFirOpen}>
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
                void handleDeleteFir();
              }}
              type="button"
              variant="destructive"
            >
              Delete FIR
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog onOpenChange={(open) => !open && setPendingId(null)} open={pendingId !== null}>
        <AlertDialogContent size="sm">
          <AlertDialogHeader>
            <AlertDialogTitle>Save changes first?</AlertDialogTitle>
            <AlertDialogDescription>
              This document has unsaved edits. Save them, or discard them and switch.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <Button onClick={() => void confirmPending("discard")} type="button" variant="outline">
              Discard
            </Button>
            <Button onClick={() => void confirmPending("save")} type="button">
              Save
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Pane>
  );
}

function WorkspaceShortcuts({
  canSave,
  onSave,
  onPrint,
  onEditDetails,
  onStepDocument,
  onToggleFieldNames,
}: {
  canSave: boolean;
  onSave: () => void;
  onPrint: () => void;
  onEditDetails: () => void;
  onStepDocument: (offset: 1 | -1) => void;
  onToggleFieldNames: () => void;
}) {
  useShortcut("save", onSave, { enabled: canSave });
  useShortcut("print", onPrint);
  useShortcut("editFir", onEditDetails);
  useShortcut("nextDocument", () => onStepDocument(1), { ignoreInputs: false });
  useShortcut("previousDocument", () => onStepDocument(-1), { ignoreInputs: false });
  useShortcut("toggleFieldNames", onToggleFieldNames);
  return null;
}

type DocumentRowProps = {
  title: string;
  active: boolean;
  printed: boolean;
  canMoveUp: boolean;
  canMoveDown: boolean;
  canRemove: boolean;
  onSelect: () => void;
  onPrintedChange: (printed: boolean) => void;
  onMove: (offset: 1 | -1) => void;
  onRemove: () => void;
};

/** A document in the FIR: print checkbox, title, and the same actions on "…" and right-click. */
function DocumentRow({
  title,
  active,
  printed,
  canMoveUp,
  canMoveDown,
  canRemove,
  onSelect,
  onPrintedChange,
  onMove,
  onRemove,
}: DocumentRowProps) {
  const actions = [
    { label: "Move up", icon: ArrowUp01Icon, disabled: !canMoveUp, run: () => onMove(-1) },
    { label: "Move down", icon: ArrowDown01Icon, disabled: !canMoveDown, run: () => onMove(1) },
  ];

  return (
    <SidebarMenuItem>
      <ContextMenu>
        <SidebarMenuButton
          isActive={active}
          onClick={onSelect}
          render={<ContextMenuTrigger render={<button type="button" />} />}
        >
          <span lang="ur" dir="rtl" className="w-full truncate text-ur">
            {title}
          </span>
        </SidebarMenuButton>
        <ContextMenuContent className="w-52">
          <ContextMenuGroup>
            <ContextMenuItem onClick={onSelect}>
              <HugeiconsIcon icon={ArrowUpRight01Icon} strokeWidth={2} />
              Open
            </ContextMenuItem>
            <ContextMenuCheckboxItem checked={printed} onCheckedChange={onPrintedChange}>
              <HugeiconsIcon icon={PrinterIcon} strokeWidth={2} />
              Include when printing
            </ContextMenuCheckboxItem>
          </ContextMenuGroup>
          <ContextMenuSeparator />
          <ContextMenuGroup>
            {actions.map((action) => (
              <ContextMenuItem key={action.label} disabled={action.disabled} onClick={action.run}>
                <HugeiconsIcon icon={action.icon} strokeWidth={2} />
                {action.label}
              </ContextMenuItem>
            ))}
          </ContextMenuGroup>
          <ContextMenuSeparator />
          <ContextMenuGroup>
            <ContextMenuItem disabled={!canRemove} variant="destructive" onClick={onRemove}>
              <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
              Remove from FIR
            </ContextMenuItem>
          </ContextMenuGroup>
        </ContextMenuContent>
      </ContextMenu>
      <SidebarMenuCheckbox
        aria-label={`Print ${title}`}
        checked={printed}
        onCheckedChange={(checked) => onPrintedChange(checked)}
      />
      <DropdownMenu>
        <DropdownMenuTrigger
          render={<SidebarMenuAction showOnHover aria-label={`Actions for ${title}`} />}
        >
          <HugeiconsIcon icon={MoreHorizontalIcon} strokeWidth={2} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-48">
          <DropdownMenuGroup>
            {actions.map((action) => (
              <DropdownMenuItem key={action.label} disabled={action.disabled} onClick={action.run}>
                <HugeiconsIcon icon={action.icon} strokeWidth={2} />
                {action.label}
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
          <DropdownMenuSeparator />
          <DropdownMenuGroup>
            <DropdownMenuItem disabled={!canRemove} variant="destructive" onClick={onRemove}>
              <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
              Remove from FIR
            </DropdownMenuItem>
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>
    </SidebarMenuItem>
  );
}

function EditorSkeleton() {
  return (
    <div className="flex h-full flex-col" aria-busy="true">
      <div className="h-10 shrink-0 border-b" />
      <div className="flex flex-1 justify-center bg-canvas pt-8">
        <Skeleton className="h-full w-[min(210mm,100%)]" />
      </div>
    </div>
  );
}

export function FirNotFound() {
  return (
    <Pane>
      <PaneHeader />
      <PaneBody className="flex">
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} strokeWidth={2} />
            </EmptyMedia>
            <EmptyTitle>FIR not found</EmptyTitle>
            <EmptyDescription>It may have been deleted.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button nativeButton={false} render={<Link to="/" />} variant="outline" size="sm">
              Back to FIRs
            </Button>
          </EmptyContent>
        </Empty>
      </PaneBody>
    </Pane>
  );
}

function FirWorkspaceSkeleton() {
  return (
    <Pane aria-busy="true">
      <PaneHeader>
        <Skeleton className="h-4 w-40" />
        <Skeleton className="h-5 w-20" />
        <PaneActions>
          <Skeleton className="h-7 w-16" />
        </PaneActions>
      </PaneHeader>
      <div className="flex min-h-0 flex-1">
        <div className="flex w-66 flex-col gap-1 border-e">
          <div className="h-10 border-b" />
          <div className="flex flex-col gap-1 p-2">
            <Skeleton className="h-9" />
            <Skeleton className="h-9" />
          </div>
        </div>
        <div className="flex-1">
          <EditorSkeleton />
        </div>
      </div>
    </Pane>
  );
}

function FirWorkspaceError() {
  return (
    <Pane>
      <PaneHeader />
      <PaneBody className="flex">
        <Empty>
          <EmptyHeader>
            <EmptyTitle>Could not load this FIR</EmptyTitle>
            <EmptyDescription>Your saved documents are safe. Reload to try again.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button variant="outline" size="sm" onClick={() => window.location.reload()}>
              Reload
            </Button>
          </EmptyContent>
        </Empty>
      </PaneBody>
    </Pane>
  );
}

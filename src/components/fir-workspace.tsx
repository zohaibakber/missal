import { useRef, useState } from "react";
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
import { PrintPreview } from "#/components/print-preview";
import {
  Pane,
  PaneActions,
  PaneBody,
  PaneHeader,
  PaneStatusBar,
  PaneTitle,
  SaveButton,
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
import { toast } from "#/components/ui/toast";
import { envelopePrintPacket, printPacket, type PrintPacket } from "#/editor/html-export";
import type { EditorSessionHandle } from "#/editor/session";
import { useShortcut } from "#/hooks/use-shortcut";
import type { FieldDisplayMode } from "#/lib/field";
import { ReorderFirDocumentsInput, type FirDocumentSummary } from "#/lib/fir-document";
import { parseFirDocumentId, type FirDocumentId, type FirId } from "#/lib/ids";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { atoms } from "#/state/atoms";

const EMPTY_DOCUMENTS: readonly FirDocumentSummary[] = [];

type FirWorkspaceProps = {
  documentId?: string;
  firId: FirId;
};

export function FirWorkspace({ documentId: documentIdParam, firId }: FirWorkspaceProps) {
  const navigate = useNavigate();
  const valueContextResult = useAtomValue(atoms.firValueContextAtom(firId));
  const documentsResult = useAtomValue(atoms.firDocumentsAtom(firId));
  const removeFir = useAtomSet(atoms.removeFirAtom, { mode: "promiseExit" });
  const loadPrintDocuments = useAtomSet(atoms.loadPrintDocumentsAtom, { mode: "promiseExit" });
  const saveDocument = useAtomSet(atoms.saveFirDocumentAtom, { mode: "promiseExit" });
  const reorderDocuments = useAtomSet(atoms.reorderFirDocumentsAtom(firId), {
    mode: "promiseExit",
  });
  const removeDocument = useAtomSet(atoms.removeFirDocumentAtom(firId), { mode: "promiseExit" });
  const documents = AsyncResult.isSuccess(documentsResult)
    ? documentsResult.value
    : EMPTY_DOCUMENTS;
  const requestedDocumentId = documentIdParam ? parseFirDocumentId(documentIdParam) : undefined;
  const activeId =
    (requestedDocumentId && documents.some((document) => document.id === requestedDocumentId)
      ? requestedDocumentId
      : documents[0]?.id) ?? null;
  const activeDocumentResult = useAtomValue(atoms.firDocumentByIdAtom(activeId));
  const activeDocument =
    activeId && AsyncResult.isSuccess(activeDocumentResult) ? activeDocumentResult.value : null;
  const [addTemplatesOpen, setAddTemplatesOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewPacket, setPreviewPacket] = useState<PrintPacket | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [savePending, setSavePending] = useState(false);
  const [displayMode, setDisplayMode] = useState<FieldDisplayMode>("values");
  // Documents print unless excluded, so ones added later print by default.
  const [excludedIds, setExcludedIds] = useState<ReadonlySet<FirDocumentId>>(() => new Set());
  const [isDeleteFirOpen, setIsDeleteFirOpen] = useState(false);
  const [pendingId, setPendingId] = useState<FirDocumentId | null>(null);
  const [editorEpoch, setEditorEpoch] = useState(0);
  const sessionRef = useRef<EditorSessionHandle | null>(null);

  if (AsyncResult.isInitial(valueContextResult) || AsyncResult.isInitial(documentsResult)) {
    return <FirWorkspaceSkeleton />;
  }

  if (AsyncResult.isFailure(valueContextResult)) {
    return Option.match(Cause.findErrorOption(valueContextResult.cause), {
      onNone: () => <FirWorkspaceError />,
      onSome: (error) =>
        Match.value(error).pipe(
          Match.tag("EntityNotFound", () => <FirNotFound />),
          Match.orElse(() => <FirWorkspaceError />),
        ),
    });
  }

  if (AsyncResult.isFailure(documentsResult)) return <FirWorkspaceError />;

  const valueContext = valueContextResult.value;
  const fir = valueContext.fir;
  const presentation = valueContext.toPresentation(displayMode);
  const printTitle = `FIR ${fir.fir_no}`;
  const printIds = documents
    .filter((document) => !excludedIds.has(document.id))
    .map((document) => document.id);

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
    const current = activeDocument;
    if (!current || !session) {
      return false;
    }

    const saved = await session.runSave(async (captured) => {
      const exit = await saveDocument({ current, document: captured.envelope });
      if (Exit.isFailure(exit)) {
        toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
        return { saved: false, value: false };
      }
      return { saved: true, value: true };
    });
    return saved === true;
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

  /** Gathers the documents marked for printing, using the live editor state for the open one. */
  async function buildPrintPacket(): Promise<PrintPacket | null> {
    if (!printIds.length) {
      toast.add({ title: "Select at least one document to print", type: "warning" });
      return null;
    }

    const captured =
      activeDocument && sessionRef.current ? sessionRef.current.captureEnvelope() : null;
    const result = await loadPrintDocuments(printIds);
    if (Exit.isFailure(result)) {
      toast.add({ title: getRepositoryErrorMessage(result), type: "error" });
      return null;
    }
    return envelopePrintPacket(
      result.value.map((document) => ({
        _tag: "Envelope",
        envelope:
          document.id === activeDocument?.id && captured ? captured.envelope : document.document,
        presentation,
      })),
      printTitle,
    );
  }

  async function openPrintPreview() {
    if (!printIds.length) {
      toast.add({ title: "Select at least one document to print", type: "warning" });
      return;
    }
    setPreviewPacket(null);
    setPreviewOpen(true);
    const packet = await buildPrintPacket();
    if (packet) setPreviewPacket(packet);
    else setPreviewOpen(false);
  }

  async function handlePrint(packet: PrintPacket) {
    setPreviewOpen(false);
    const failure = await printPacket(packet);
    if (failure) {
      toast.add({ title: failure, type: "error" });
    }
  }

  function stepDocument(offset: 1 | -1) {
    const index = documents.findIndex((document) => document.id === activeId);
    const next = documents[index + offset];
    if (next) selectDocument(next.id);
  }

  function setPrinted(id: FirDocumentId, printed: boolean) {
    setExcludedIds((current) => {
      const next = new Set(current);
      if (printed) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  const printCount = printIds.length;

  return (
    <Pane>
      <WorkspaceShortcuts
        canSave={dirty && !savePending}
        onSave={() => void handleSave()}
        onPrint={() => void openPrintPreview()}
        onEditDetails={() => setDetailsOpen(true)}
        onStepDocument={stepDocument}
        onToggleFieldNames={() =>
          setDisplayMode((mode) => (mode === "labels" ? "values" : "labels"))
        }
      />
      <UnsavedChanges isDirty={() => dirty && pendingId === null} />
      <SplitView id="fir-workspace">
        <SplitViewList>
          <SplitViewListHeader>
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
                    printed={!excludedIds.has(document.id)}
                    canMoveUp={index > 0}
                    canMoveDown={index < documents.length - 1}
                    canRemove={!(dirty && document.id === activeId)}
                    onSelect={() => selectDocument(document.id)}
                    onPrintedChange={(checked) => setPrinted(document.id, checked)}
                    onMove={(offset) => void handleReorder(index, index + offset)}
                    onRemove={() => void handleRemoveDocument(document.id)}
                  />
                ))}
              </SidebarMenu>
            ) : (
              <p className="px-4 py-3 text-xs text-muted-foreground">No documents yet</p>
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
          {/* As on the template page: the editor column carries the actions, then the FIR. */}
          <Pane>
            <PaneHeader>
              <PaneActions className="ms-0 me-auto">
                {activeDocument ? (
                  <Hint label="Save document" shortcut="save">
                    <SaveButton
                      dirty={dirty}
                      pending={savePending}
                      onClick={() => void handleSave()}
                    />
                  </Hint>
                ) : null}
                <IconAction
                  label={
                    printCount
                      ? `Print ${printCount} ${printCount === 1 ? "document" : "documents"}…`
                      : "Print…"
                  }
                  shortcut="print"
                  disabled={!printCount}
                  onClick={() => void openPrintPreview()}
                >
                  <HugeiconsIcon icon={PrinterIcon} strokeWidth={2} />
                </IconAction>
                <IconAction
                  label="Edit FIR details"
                  shortcut="editFir"
                  onClick={() => setDetailsOpen(true)}
                >
                  <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
                </IconAction>
                <DropdownMenu>
                  <Hint label="More actions">
                    <DropdownMenuTrigger
                      render={
                        <Button
                          size="icon-sm"
                          type="button"
                          variant="ghost"
                          aria-label="More actions"
                        />
                      }
                    >
                      <HugeiconsIcon icon={MoreHorizontalIcon} strokeWidth={2} />
                    </DropdownMenuTrigger>
                  </Hint>
                  <DropdownMenuContent align="start" className="w-72">
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
                    </DropdownMenuGroup>
                    <DropdownMenuSeparator />
                    <DropdownMenuGroup>
                      <DropdownMenuItem
                        onClick={() => setIsDeleteFirOpen(true)}
                        variant="destructive"
                      >
                        <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                        Delete FIR
                      </DropdownMenuItem>
                    </DropdownMenuGroup>
                  </DropdownMenuContent>
                </DropdownMenu>
              </PaneActions>
              <span
                lang="ur"
                dir="rtl"
                className="hidden min-w-0 truncate text-ur text-muted-foreground lg:block"
              >
                {fir.offence}
              </span>
              <FirStatusBadge status={fir.status} />
              <PaneTitle className="flex-none">FIR {fir.fir_no}</PaneTitle>
            </PaneHeader>
            {activeDocument ? (
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
          </Pane>
        </SplitViewDetail>
      </SplitView>

      <EditFirSheet fir={fir} open={detailsOpen} onOpenChange={setDetailsOpen} />

      <PrintPreview
        open={previewOpen}
        onOpenChange={setPreviewOpen}
        title={printTitle}
        packet={previewPacket}
        description={`${printCount} ${printCount === 1 ? "document" : "documents"}`}
        onPrint={() => {
          if (previewPacket) void handlePrint(previewPacket);
        }}
      />

      <AlertDialog onOpenChange={setIsDeleteFirOpen} open={isDeleteFirOpen}>
        <AlertDialogContent>
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
        <AlertDialogContent>
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
      <div className="flex min-h-0 flex-1">
        <div className="flex w-66 flex-col border-e">
          <div className="h-12 border-b" />
          <div className="flex flex-col gap-1 p-2">
            <Skeleton className="h-9" />
            <Skeleton className="h-9" />
          </div>
        </div>
        <Pane>
          <PaneHeader>
            <PaneActions className="ms-0 me-auto">
              <Skeleton className="h-7 w-16" />
            </PaneActions>
            <Skeleton className="h-5 w-20" />
            <Skeleton className="h-4 w-40" />
          </PaneHeader>
          <EditorSkeleton />
        </Pane>
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

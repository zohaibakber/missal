import { useEffect, useMemo, useRef, useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Cause, Exit, Match, Option } from "effect";
import { Link, useNavigate } from "@tanstack/react-router";
import { EditFirForm } from "#/components/create-fir-form";
import { FirTemplatePicker } from "#/components/fir-template-picker";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { DirectionProvider } from "#/components/ui/direction";
import {
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarGroup,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
} from "#/components/ui/sidebar";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "#/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "#/components/ui/tabs";
import { toast } from "#/components/ui/toast";
import {
  ArrowDown01Icon,
  ArrowUp01Icon,
  Delete02Icon,
  LegalDocument01Icon,
  MoreVerticalIcon,
  PrinterIcon,
} from "@hugeicons/core-free-icons";
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
} from "#/components/ui/alert-dialog";
import { Badge } from "#/components/ui/badge";
import { Button } from "#/components/ui/button";
import { Checkbox } from "#/components/ui/checkbox";
import { DocumentEditor } from "#/components/document-editor";
import {
  DropdownMenu,
  DropdownMenuGroup,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
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
import { Skeleton } from "#/components/ui/skeleton";
import { printEnvelopePacket } from "#/editor/html-export";
import type { EditorSessionHandle } from "#/editor/session";
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
  const [sidebarView, setSidebarView] = useState<"documents" | "templates" | null>(null);
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
  const visibleSidebar = sidebarView ?? (documents.length ? "documents" : "templates");

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

  return (
    <DirectionProvider direction="ltr">
      <main
        dir="ltr"
        className="flex h-full min-h-0 flex-col"
        onKeyDown={(event) => {
          if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
            event.preventDefault();
            if (dirty && !savePending) void handleSave();
          }
        }}
      >
        <UnsavedChanges isDirty={() => dirty && pendingId === null} />
        <section className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b px-4 py-3">
          <div className="flex min-w-0 flex-col gap-2">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="text-base font-medium">FIR {fir.fir_no}</h1>
              <Badge variant="outline">{fir.status}</Badge>
            </div>
            <p
              lang="ur"
              dir="rtl"
              className="line-clamp-1 text-base leading-loose text-muted-foreground"
            >
              {activeDocument?.title ?? fir.offence}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="outline" onClick={() => setDetailsOpen(true)}>
              Edit FIR details
            </Button>
            <Button variant="outline" disabled={!outputIds.size} onClick={() => void handlePrint()}>
              <HugeiconsIcon icon={PrinterIcon} data-icon="inline-start" />
              Print ({outputIds.size})
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger
                render={<Button size="icon-sm" type="button" variant="outline" />}
              >
                <HugeiconsIcon icon={MoreVerticalIcon} strokeWidth={2} />
                <span className="sr-only">Open FIR actions</span>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-48">
                <DropdownMenuGroup>
                  <DropdownMenuCheckboxItem
                    checked={displayMode === "labels"}
                    onCheckedChange={(checked) => setDisplayMode(checked ? "labels" : "values")}
                  >
                    Show field names
                  </DropdownMenuCheckboxItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={() => setIsDeleteFirOpen(true)} variant="destructive">
                    <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                    Delete FIR
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
            {activeDocument && (
              <span role="status" className="text-xs text-muted-foreground">
                {dirty ? "Unsaved" : "Saved"}
              </span>
            )}
            <Button
              disabled={!dirty || savePending}
              onClick={() => void handleSave()}
              type="button"
            >
              {savePending ? "Saving…" : "Save"}
            </Button>
          </div>
        </section>

        <section className="flex min-h-0 flex-1 flex-col overflow-hidden md:flex-row">
          <Sidebar
            collapsible="none"
            className="max-h-64 w-full shrink-0 border-b md:h-full md:max-h-none md:order-last md:w-64 md:border-b-0 md:border-l"
          >
            <Tabs
              className="min-h-0 flex-1 gap-0"
              value={visibleSidebar}
              onValueChange={(view) => {
                if (view === "documents" || view === "templates") setSidebarView(view);
              }}
            >
              <SidebarHeader className="px-2 py-1">
                <TabsList variant="line" aria-label="Sidebar view">
                  <TabsTrigger value="documents">Documents ({documents.length})</TabsTrigger>
                  <TabsTrigger value="templates">Templates</TabsTrigger>
                </TabsList>
              </SidebarHeader>
              <SidebarContent>
                <TabsContent value="documents">
                  <SidebarGroup>
                    <SidebarMenu>
                      {documents.map((document, index) => (
                        <SidebarMenuItem key={document.id} className="flex items-center gap-1">
                          <Checkbox
                            aria-label={`Include ${document.title} in print`}
                            checked={outputIds.has(document.id)}
                            onCheckedChange={(checked) =>
                              setOutputIds((current) => {
                                const next = new Set(current);
                                if (checked) next.add(document.id);
                                else next.delete(document.id);
                                return next;
                              })
                            }
                          />
                          <SidebarMenuButton
                            className="min-w-0 flex-1"
                            isActive={document.id === activeId}
                            onClick={() => selectDocument(document.id)}
                          >
                            <span
                              lang="ur"
                              dir="rtl"
                              className="w-full truncate text-base leading-loose"
                            >
                              {document.title}
                            </span>
                          </SidebarMenuButton>
                          <DropdownMenu>
                            <DropdownMenuTrigger
                              render={
                                <Button
                                  variant="ghost"
                                  size="icon-sm"
                                  aria-label={`Actions for ${document.title}`}
                                />
                              }
                            >
                              <HugeiconsIcon icon={MoreVerticalIcon} />
                            </DropdownMenuTrigger>
                            <DropdownMenuContent align="end">
                              <DropdownMenuGroup>
                                <DropdownMenuItem
                                  disabled={index === 0}
                                  onClick={() => void handleReorder(index, index - 1)}
                                >
                                  <HugeiconsIcon icon={ArrowUp01Icon} />
                                  Move up
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  disabled={index === documents.length - 1}
                                  onClick={() => void handleReorder(index, index + 1)}
                                >
                                  <HugeiconsIcon icon={ArrowDown01Icon} />
                                  Move down
                                </DropdownMenuItem>
                                <DropdownMenuSeparator />
                                <DropdownMenuItem
                                  disabled={dirty && document.id === activeId}
                                  variant="destructive"
                                  onClick={() => void handleRemoveDocument(document.id)}
                                >
                                  <HugeiconsIcon icon={Delete02Icon} />
                                  Remove document
                                </DropdownMenuItem>
                              </DropdownMenuGroup>
                            </DropdownMenuContent>
                          </DropdownMenu>
                        </SidebarMenuItem>
                      ))}
                    </SidebarMenu>
                    {!documents.length && (
                      <p className="px-2 py-3 text-xs text-muted-foreground">
                        Your documents will appear here.
                      </p>
                    )}
                  </SidebarGroup>
                </TabsContent>
                <TabsContent value="templates">
                  <FirTemplatePicker
                    firId={firId}
                    onAdded={(id) => {
                      setSidebarView("documents");
                      selectDocument(id);
                    }}
                  />
                </TabsContent>
              </SidebarContent>
            </Tabs>
          </Sidebar>
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <div className="min-h-0 flex-1">
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
                <Skeleton className="h-full min-h-64" />
              ) : (
                <Empty className="h-full min-h-64">
                  <EmptyHeader>
                    <EmptyMedia variant="icon">
                      <HugeiconsIcon icon={LegalDocument01Icon} />
                    </EmptyMedia>
                    <EmptyTitle>Start with a template</EmptyTitle>
                    <EmptyDescription>
                      Choose a template from the sidebar to start writing.
                    </EmptyDescription>
                  </EmptyHeader>
                </Empty>
              )}
            </div>
          </div>
        </section>
        <Sheet open={detailsOpen} onOpenChange={setDetailsOpen}>
          <SheetContent className="data-[side=right]:sm:max-w-2xl" dir="ltr">
            <SheetHeader>
              <SheetTitle>Edit FIR {fir.fir_no}</SheetTitle>
              <SheetDescription>
                Update case details used by the documents in this FIR.
              </SheetDescription>
            </SheetHeader>
            <div className="min-h-0 flex-1 overflow-y-auto px-4" dir="rtl">
              <EditFirForm
                fir={fir}
                onSuccess={() => {
                  setDetailsOpen(false);
                  toast.add({ title: "FIR details saved", type: "success" });
                }}
              />
            </div>
          </SheetContent>
        </Sheet>

        <AlertDialog onOpenChange={setIsDeleteFirOpen} open={isDeleteFirOpen}>
          <AlertDialogContent size="sm">
            <AlertDialogHeader>
              <AlertDialogTitle>Delete FIR</AlertDialogTitle>
              <AlertDialogDescription>
                This removes FIR {fir.fir_no} and every document copied from templates.
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
                <HugeiconsIcon data-icon="inline-start" icon={Delete02Icon} />
                Delete
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>

        <AlertDialog onOpenChange={(open) => !open && setPendingId(null)} open={pendingId !== null}>
          <AlertDialogContent size="sm">
            <AlertDialogHeader>
              <AlertDialogTitle>Unsaved changes</AlertDialogTitle>
              <AlertDialogDescription>
                Save this document, discard the draft, or stay on the current document.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <Button
                onClick={() => void confirmPending("discard")}
                type="button"
                variant="outline"
              >
                Discard
              </Button>
              <Button onClick={() => void confirmPending("save")} type="button">
                Save
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </main>
    </DirectionProvider>
  );
}

export function FirNotFound() {
  return (
    <main className="p-4 lg:p-6">
      <Empty className="min-h-[28rem] border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <HugeiconsIcon icon={LegalDocument01Icon} />
          </EmptyMedia>
          <EmptyTitle>FIR not found</EmptyTitle>
          <EmptyDescription>This FIR may have been removed.</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button nativeButton={false} render={<Link to="/" />} variant="outline">
            Back to dataset
          </Button>
        </EmptyContent>
      </Empty>
    </main>
  );
}

function FirWorkspaceSkeleton() {
  return (
    <main className="flex flex-col gap-4 p-4 lg:p-6">
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-2">
          <Skeleton className="h-8 w-24" />
          <Skeleton className="h-7 w-40" />
          <Skeleton className="h-4 w-64" />
        </div>
        <Skeleton className="h-8 w-24" />
      </div>
      <div className="grid gap-4 xl:grid-cols-[minmax(16rem,20rem)_minmax(0,1fr)]">
        <Skeleton className="h-[42rem]" />
        <Skeleton className="h-[42rem]" />
      </div>
    </main>
  );
}

function FirWorkspaceError() {
  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyTitle>Could not load this FIR</EmptyTitle>
        <EmptyDescription>
          Try reloading the workspace. Your saved documents are kept in the database.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button variant="outline" onClick={() => window.location.reload()}>
          Reload workspace
        </Button>
      </EmptyContent>
    </Empty>
  );
}

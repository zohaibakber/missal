import { useEffect, useMemo, useRef, useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit } from "effect";
import { Link, useNavigate } from "@tanstack/react-router";
import { toast } from "#/components/ui/toast";
import {
  Delete02Icon,
  FileImportIcon,
  PrinterIcon,
  LegalDocument01Icon,
  MoreHorizontalIcon,
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
import { Hint } from "#/components/hint";
import { IconAction } from "#/components/icon-action";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { PrintPreview } from "#/components/print-preview";
import { Pane, PaneActions, PaneBody, PaneHeader, SaveButton } from "#/components/pane";
import { UnsavedChanges } from "#/components/unsaved-changes";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
import { Spinner } from "#/components/ui/spinner";
import { Button } from "#/components/ui/button";
import { DocumentEditor } from "#/components/document-editor";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import { Input } from "#/components/ui/input";
import { Skeleton } from "#/components/ui/skeleton";
import { catalogFieldPresentation } from "#/lib/field";
import { emptyDocumentEnvelope, projectDocument } from "#/lib/document-format";
import { type TemplateId } from "#/lib/ids";
import { indexPlaceholders, type PlaceholderIndex } from "#/lib/placeholder";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { TemplateCreateInput, TemplateRecord, TemplateUpdateInput } from "#/lib/templates";
import { envelopePrintPacket, printPacket, type PrintPacket } from "#/editor/html-export";
import type { EditorSessionHandle } from "#/editor/session";
import { useShortcut } from "#/hooks/use-shortcut";
import { atoms } from "#/state/atoms";

type TemplateEditorFormProps = {
  templateId?: TemplateId;
};

export function TemplateEditorForm({ templateId }: TemplateEditorFormProps) {
  if (templateId) {
    return <EditTemplateForm templateId={templateId} />;
  }

  return <CreateTemplateForm />;
}

function CreateTemplateForm() {
  const placeholderIndexResult = useAtomValue(atoms.placeholderIndexAtom);
  const placeholderIndex = AsyncResult.isSuccess(placeholderIndexResult)
    ? placeholderIndexResult.value
    : indexPlaceholders([]);

  return <TemplateEditorWorkspace placeholderIndex={placeholderIndex} selectedTemplate={null} />;
}

function EditTemplateForm({ templateId }: { templateId: TemplateId }) {
  const placeholderIndexResult = useAtomValue(atoms.placeholderIndexAtom);
  const templateResult = useAtomValue(atoms.templateByIdAtom(templateId));
  const placeholderIndex = AsyncResult.isSuccess(placeholderIndexResult)
    ? placeholderIndexResult.value
    : indexPlaceholders([]);

  if (AsyncResult.isFailure(templateResult)) {
    return <TemplateNotFound />;
  }

  if (AsyncResult.isInitial(templateResult)) {
    return (
      <Pane aria-busy="true">
        <PaneHeader>
          <Skeleton className="h-5 w-48" />
          <PaneActions>
            <Skeleton className="h-7 w-16" />
          </PaneActions>
        </PaneHeader>
        <div className="h-10 border-b" />
        <div className="flex-1 bg-canvas" />
      </Pane>
    );
  }

  return (
    <TemplateEditorWorkspace
      placeholderIndex={placeholderIndex}
      selectedTemplate={templateResult.value}
    />
  );
}

export function TemplateNotFound() {
  return (
    <Pane>
      <PaneHeader />
      <PaneBody className="flex">
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} strokeWidth={2} />
            </EmptyMedia>
            <EmptyTitle>Template not found</EmptyTitle>
            <EmptyDescription>It may have been deleted.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              nativeButton={false}
              render={<Link to="/templates" />}
              variant="outline"
              size="sm"
            >
              Back to templates
            </Button>
          </EmptyContent>
        </Empty>
      </PaneBody>
    </Pane>
  );
}

function TemplateEditorWorkspace({
  placeholderIndex,
  selectedTemplate,
}: {
  placeholderIndex: PlaceholderIndex;
  selectedTemplate: TemplateRecord | null;
}) {
  const navigate = useNavigate();
  const createTemplate = useAtomSet(atoms.createTemplateAtom, { mode: "promiseExit" });
  const saveTemplate = useAtomSet(atoms.saveTemplateAtom, { mode: "promiseExit" });
  const removeTemplate = useAtomSet(atoms.removeTemplateAtom, { mode: "promiseExit" });
  const [name, setName] = useState("");
  const [revision, setRevision] = useState(selectedTemplate?.revision);
  const [contentDirty, setContentDirty] = useState(false);
  const [savedName, setSavedName] = useState(selectedTemplate?.name ?? "");
  const dirty = contentDirty || name !== savedName;
  const [savePending, setSavePending] = useState(false);
  const [importPending, setImportPending] = useState(false);
  const [pendingImport, setPendingImport] = useState<File | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewPacket, setPreviewPacket] = useState<PrintPacket | null>(null);
  const allowNavigationRef = useRef(false);
  const sessionRef = useRef<EditorSessionHandle | null>(null);
  const loadedKeyRef = useRef<string | number | null>(null);
  const presentation = useMemo(
    () => catalogFieldPresentation(placeholderIndex),
    [placeholderIndex],
  );
  const sessionKey = selectedTemplate?.id ?? "new";

  useEffect(() => {
    if (loadedKeyRef.current === sessionKey) {
      return;
    }

    loadedKeyRef.current = sessionKey;
    allowNavigationRef.current = false;
    setName(selectedTemplate?.name ?? "");
    setRevision(selectedTemplate?.revision);
    setContentDirty(false);
    setSavedName(selectedTemplate?.name ?? "");
  }, [selectedTemplate, sessionKey]);

  async function handleImport(file: File) {
    const session = sessionRef.current;
    if (!session || importPending || savePending) return;
    setPendingImport(null);
    setImportPending(true);
    try {
      const notices = await session.importDocx(file);
      if (sessionRef.current !== session) return;
      setName((current) => (current.trim() ? current : file.name.replace(/\.docx$/i, "")));
      toast.add({
        title: "Word document imported",
        description: notices.length
          ? notices.join(" ")
          : "Review the layout and placeholders, then save the template.",
        type: notices.length ? "warning" : "success",
      });
    } catch (error) {
      toast.add({
        title: "Could not import Word document",
        description:
          error instanceof Error ? error.message : "Choose a valid .docx file and try again.",
        type: "error",
      });
    } finally {
      setImportPending(false);
    }
  }

  function chooseImport(file: File) {
    const session = sessionRef.current;
    if (!session) return;
    const projection = projectDocument(session.captureEnvelope().envelope);
    if (contentDirty || selectedTemplate || projection.plainText || projection.fieldCount) {
      setPendingImport(file);
      return;
    }
    void handleImport(file);
  }

  async function handleSave() {
    const trimmed = name.trim();
    const session = sessionRef.current;
    if (!trimmed || !session) {
      return;
    }

    if (selectedTemplate && revision === undefined) {
      return;
    }

    if (!session.tryBeginSave()) {
      return;
    }

    const captured = session.captureEnvelope();
    try {
      if (selectedTemplate && revision !== undefined) {
        const exit = await saveTemplate(
          new TemplateUpdateInput({
            document: captured.envelope,
            expectedRevision: revision,
            id: selectedTemplate.id,
            name: trimmed,
          }),
        );

        if (Exit.isFailure(exit)) {
          toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
          return;
        }

        session.markSaved(captured.contentRevision);
        setRevision(exit.value.revision);
        setSavedName(trimmed);
        toast.add({ title: "Template updated", type: "success" });
        return;
      }

      const exit = await createTemplate(
        new TemplateCreateInput({
          document: captured.envelope,
          name: trimmed,
        }),
      );

      if (Exit.isFailure(exit)) {
        toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
        return;
      }

      allowNavigationRef.current = true;
      void navigate({
        params: { templateId: `${exit.value.id}` },
        to: "/templates/$templateId",
      });
    } finally {
      session.endSave();
    }
  }

  async function handleDelete() {
    if (!selectedTemplate) {
      return;
    }

    const exit = await removeTemplate(selectedTemplate.id);

    if (Exit.isFailure(exit)) {
      toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
      return;
    }

    allowNavigationRef.current = true;
    setIsDeleteDialogOpen(false);
    void navigate({ to: "/templates" });
  }

  const printTitle = name.trim() || "Untitled template";

  function openPrintPreview() {
    const session = sessionRef.current;
    if (!session) return;
    setPreviewPacket(
      envelopePrintPacket(
        [{ _tag: "Envelope", envelope: session.captureEnvelope().envelope, presentation }],
        printTitle,
      ),
    );
    setPreviewOpen(true);
  }

  const canSave =
    Boolean(name.trim()) && !savePending && !importPending && (selectedTemplate === null || dirty);

  return (
    <Pane>
      <TemplateEditorShortcuts
        templateId={selectedTemplate?.id}
        canSave={canSave}
        onSave={() => void handleSave()}
        onImport={() => fileInputRef.current?.click()}
        onPrint={openPrintPreview}
      />
      <UnsavedChanges isDirty={() => dirty && !allowNavigationRef.current} />
      <PaneHeader>
        <PaneActions className="ms-0 me-auto">
          <Hint label={selectedTemplate ? "Save template" : "Create template"} shortcut="save">
            <SaveButton
              dirty={selectedTemplate === null || dirty}
              pending={savePending}
              disabled={!canSave}
              label={selectedTemplate ? "Save" : "Create"}
              onClick={() => void handleSave()}
            />
          </Hint>
          <input
            ref={fileInputRef}
            type="file"
            accept=".docx,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            aria-label="Upload Word document"
            hidden
            onChange={(event) => {
              const file = event.target.files?.[0];
              event.target.value = "";
              if (file) chooseImport(file);
            }}
          />
          <IconAction label="Print preview…" shortcut="print" onClick={openPrintPreview}>
            <HugeiconsIcon icon={PrinterIcon} strokeWidth={2} />
          </IconAction>
          <DropdownMenu>
            <Hint label="More actions">
              <DropdownMenuTrigger
                render={
                  <Button size="icon-sm" type="button" variant="ghost" aria-label="More actions" />
                }
              >
                {importPending ? (
                  <Spinner />
                ) : (
                  <HugeiconsIcon icon={MoreHorizontalIcon} strokeWidth={2} />
                )}
              </DropdownMenuTrigger>
            </Hint>
            <DropdownMenuContent align="start" className="w-48">
              <DropdownMenuGroup>
                <DropdownMenuItem
                  disabled={savePending || importPending}
                  onClick={() => fileInputRef.current?.click()}
                >
                  <HugeiconsIcon icon={FileImportIcon} strokeWidth={2} />
                  Upload Word
                  <DropdownMenuShortcut>
                    <ShortcutKbd id="importDocx" />
                  </DropdownMenuShortcut>
                </DropdownMenuItem>
              </DropdownMenuGroup>
              {selectedTemplate ? (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    <DropdownMenuItem
                      disabled={savePending || importPending}
                      onClick={() => setIsDeleteDialogOpen(true)}
                      variant="destructive"
                    >
                      <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                      Delete template
                    </DropdownMenuItem>
                  </DropdownMenuGroup>
                </>
              ) : null}
            </DropdownMenuContent>
          </DropdownMenu>
        </PaneActions>
        <Input
          aria-label="Template name"
          id="template-name"
          lang="ur"
          dir="rtl"
          autoFocus={!selectedTemplate}
          placeholder="ٹیمپلیٹ کا نام"
          variant="title"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
      </PaneHeader>
      <DocumentEditor
        aria-label="Template content"
        document={selectedTemplate?.document ?? emptyDocumentEnvelope()}
        onDirtyChange={setContentDirty}
        onSavePendingChange={setSavePending}
        onSessionReady={(session) => {
          sessionRef.current = session;
        }}
        placeholderIndex={placeholderIndex}
        presentation={presentation}
        sessionKey={sessionKey}
      />

      <PrintPreview
        open={previewOpen}
        onOpenChange={setPreviewOpen}
        title={printTitle}
        packet={previewPacket}
        description="Placeholders show their names"
        onPrint={() => {
          setPreviewOpen(false);
          if (previewPacket && !printPacket(previewPacket)) {
            toast.add({ title: "Unable to prepare print view", type: "error" });
          }
        }}
      />

      <AlertDialog
        open={pendingImport !== null}
        onOpenChange={(open) => {
          if (!open) setPendingImport(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Replace template content?</AlertDialogTitle>
            <AlertDialogDescription>
              Importing replaces all text and formatting in this editor. The saved template stays
              unchanged until you save.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              type="button"
              onClick={() => {
                if (pendingImport) void handleImport(pendingImport);
              }}
            >
              Import and replace
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <AlertDialog onOpenChange={setIsDeleteDialogOpen} open={isDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete this template?</AlertDialogTitle>
            <AlertDialogDescription>
              This can't be undone. Templates that an FIR's documents use can't be deleted.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(event) => {
                event.preventDefault();
                void handleDelete();
              }}
              type="button"
              variant="destructive"
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Pane>
  );
}

function TemplateEditorShortcuts({
  templateId,
  canSave,
  onSave,
  onImport,
  onPrint,
}: {
  templateId: TemplateId | undefined;
  canSave: boolean;
  onSave: () => void;
  onImport: () => void;
  onPrint: () => void;
}) {
  const navigate = useNavigate();
  const templates = useAtomValue(atoms.templatesAtom);

  function step(offset: 1 | -1) {
    if (!AsyncResult.isSuccess(templates)) return;
    const list = templates.value;
    const index = list.findIndex((template) => template.id === templateId);
    const next = list[index === -1 ? 0 : index + offset];
    if (next) {
      void navigate({ to: "/templates/$templateId", params: { templateId: `${next.id}` } });
    }
  }

  useShortcut("save", onSave, { enabled: canSave });
  useShortcut("importDocx", onImport);
  useShortcut("print", onPrint);
  useShortcut("nextTemplate", () => step(1), { ignoreInputs: false });
  useShortcut("previousTemplate", () => step(-1), { ignoreInputs: false });
  return null;
}

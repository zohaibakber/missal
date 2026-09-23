import { useEffect, useMemo, useRef, useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit } from "effect";
import { Link, useNavigate } from "@tanstack/react-router";
import { toast } from "#/components/ui/toast";
import {
  Delete02Icon,
  FileImportIcon,
  LegalDocument01Icon,
  MoreVerticalIcon,
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
import { IconAction } from "#/components/icon-action";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { SaveStatus, WorkspaceHeader } from "#/components/workspace";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "#/components/ui/dropdown-menu";
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
    return (
      <div className="p-6">
        <Empty className="min-h-[28rem]" variant="outline">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} />
            </EmptyMedia>
            <EmptyTitle>Template not found</EmptyTitle>
            <EmptyDescription>This template may have been removed.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button nativeButton={false} render={<Link to="/templates" />} variant="outline">
              Back to templates
            </Button>
          </EmptyContent>
        </Empty>
      </div>
    );
  }

  if (AsyncResult.isInitial(templateResult)) {
    return (
      <div className="flex flex-col gap-4 p-6">
        <Skeleton className="h-8 w-40" />
        <Skeleton className="h-[42rem]" />
      </div>
    );
  }

  return (
    <TemplateEditorWorkspace
      placeholderIndex={placeholderIndex}
      selectedTemplate={templateResult.value}
    />
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

  const canSave =
    Boolean(name.trim()) && !savePending && !importPending && (selectedTemplate === null || dirty);

  return (
    <div className="flex h-full min-h-96 min-w-0 flex-col">
      <TemplateEditorShortcuts
        templateId={selectedTemplate?.id}
        canSave={canSave}
        onSave={() => void handleSave()}
        onImport={() => fileInputRef.current?.click()}
      />
      <UnsavedChanges isDirty={() => dirty && !allowNavigationRef.current} />
      <WorkspaceHeader>
        <Input
          aria-label="Template name"
          id="template-name"
          lang="ur"
          dir="auto"
          autoFocus={!selectedTemplate}
          className="min-w-48 flex-1"
          placeholder="ٹیمپلیٹ کا نام"
          variant="plain"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <div className="flex items-center gap-2">
          <SaveStatus dirty={dirty} />
          <input
            ref={fileInputRef}
            type="file"
            accept=".docx,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            aria-label="Import Word document"
            hidden
            onChange={(event) => {
              const file = event.target.files?.[0];
              event.target.value = "";
              if (file) chooseImport(file);
            }}
          />
          <IconAction
            label={importPending ? "Importing…" : "Import Word document"}
            shortcut="importDocx"
            disabled={savePending || importPending}
            onClick={() => fileInputRef.current?.click()}
          >
            <HugeiconsIcon icon={FileImportIcon} />
          </IconAction>
          {selectedTemplate ? (
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button size="icon-sm" type="button" variant="ghost" aria-label="More actions" />
                }
              >
                <HugeiconsIcon icon={MoreVerticalIcon} strokeWidth={2} />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-44">
                <DropdownMenuItem
                  disabled={savePending || importPending}
                  onClick={() => setIsDeleteDialogOpen(true)}
                  variant="destructive"
                >
                  <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                  Delete template
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : null}
          <Button disabled={!canSave} onClick={() => void handleSave()} size="sm" type="button">
            {savePending ? "Saving…" : selectedTemplate ? "Save" : "Create template"}
            <ShortcutKbd id="save" />
          </Button>
        </div>
      </WorkspaceHeader>
      <section className="min-h-0 min-w-0 flex-1">
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
      </section>

      <AlertDialog
        open={pendingImport !== null}
        onOpenChange={(open) => {
          if (!open) setPendingImport(null);
        }}
      >
        <AlertDialogContent size="sm">
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
        <AlertDialogContent size="sm">
          <AlertDialogHeader>
            <AlertDialogTitle>Delete template</AlertDialogTitle>
            <AlertDialogDescription>
              Templates in use by saved FIR documents cannot be deleted.
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
              <HugeiconsIcon data-icon="inline-start" icon={Delete02Icon} />
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function TemplateEditorShortcuts({
  templateId,
  canSave,
  onSave,
  onImport,
}: {
  templateId: TemplateId | undefined;
  canSave: boolean;
  onSave: () => void;
  onImport: () => void;
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
  useShortcut("nextTemplate", () => step(1), { ignoreInputs: false });
  useShortcut("previousTemplate", () => step(-1), { ignoreInputs: false });
  return null;
}

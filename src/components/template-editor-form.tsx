import { useEffect, useMemo, useRef, useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit } from "effect";
import { Link, useNavigate } from "@tanstack/react-router";
import { toast } from "#/components/ui/toast";
import { Delete02Icon, LegalDocument01Icon } from "@hugeicons/core-free-icons";
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
import { UnsavedChanges } from "#/components/unsaved-changes";
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
import { emptyDocumentEnvelope } from "#/lib/document-format";
import { type TemplateId } from "#/lib/ids";
import { indexPlaceholders, type PlaceholderIndex } from "#/lib/placeholder";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { TemplateCreateInput, TemplateRecord, TemplateUpdateInput } from "#/lib/templates";
import type { EditorSessionHandle } from "#/editor/session";
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
      <main className="p-4 lg:p-6">
        <Empty className="min-h-[28rem] border">
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
      </main>
    );
  }

  if (AsyncResult.isInitial(templateResult) || AsyncResult.isWaiting(templateResult)) {
    return (
      <main className="flex flex-col gap-4 p-4">
        <Skeleton className="h-8 w-40" />
        <Skeleton className="h-[42rem]" />
      </main>
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

  return (
    <main className="flex h-full min-h-96 min-w-0 flex-col" dir="rtl">
      <UnsavedChanges isDirty={() => dirty && !allowNavigationRef.current} />
      <section className="flex shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
        <Input
          aria-label="Template name"
          id="template-name"
          lang="ur"
          className="min-w-40 flex-1 rounded-none border-0 bg-transparent px-0 shadow-none focus-visible:ring-0 dark:bg-transparent"
          placeholder="ٹیمپلیٹ کا نام"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <Button
          disabled={!name.trim() || savePending || (selectedTemplate !== null && !dirty)}
          onClick={() => void handleSave()}
          type="button"
        >
          {savePending ? "Saving…" : "Save"}
        </Button>
        {selectedTemplate ? (
          <Button
            aria-label="Delete template"
            onClick={() => setIsDeleteDialogOpen(true)}
            size="icon-sm"
            type="button"
            variant="ghost"
          >
            <HugeiconsIcon icon={Delete02Icon} />
          </Button>
        ) : null}
      </section>
      <section className="min-h-0 min-w-0 flex-1">
        <DocumentEditor
          aria-label="ٹیمپلیٹ کا متن"
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
    </main>
  );
}

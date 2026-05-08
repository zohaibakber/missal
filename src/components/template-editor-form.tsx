import { useEffect, useMemo, useState } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { Link, useNavigate } from "@tanstack/react-router";
import { toast } from "sonner";
import {
  ArrowDown01Icon,
  ArrowUp01Icon,
  Delete02Icon,
  FileEditIcon,
  LegalDocument01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { getNextTemplateId, templateCollection } from "#/db-collections";
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
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "#/components/ui/breadcrumb";
import { Button } from "#/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupInput } from "#/components/ui/input-group";
import { TemplateRichEditor } from "#/components/template-rich-editor";
import { extractPlaceholders } from "#/lib/templates";

type TemplateEditorFormProps = {
  templateId?: number;
};

function createDraft() {
  return {
    content: "",
    name: "",
  };
}

export function TemplateEditorForm({ templateId }: TemplateEditorFormProps) {
  const navigate = useNavigate();
  const { data: templates } = useLiveQuery(templateCollection);
  const selectedTemplate =
    templateId === undefined
      ? null
      : (templates.find((template) => template.id === templateId) ?? null);
  const sortedTemplates = useMemo(
    () => [...templates].sort((first, second) => first.id - second.id),
    [templates],
  );
  const selectedTemplateIndex = selectedTemplate
    ? sortedTemplates.findIndex((template) => template.id === selectedTemplate.id)
    : -1;
  const previousTemplate =
    selectedTemplateIndex > 0 ? sortedTemplates[selectedTemplateIndex - 1] : null;
  const nextTemplate =
    selectedTemplateIndex >= 0 && selectedTemplateIndex < sortedTemplates.length - 1
      ? sortedTemplates[selectedTemplateIndex + 1]
      : null;
  const [draft, setDraft] = useState(createDraft);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const placeholders = useMemo(() => extractPlaceholders(draft.content), [draft.content]);
  const isEditing = templateId !== undefined;

  useEffect(() => {
    if (selectedTemplate) {
      setDraft({
        content: selectedTemplate.content,
        name: selectedTemplate.name,
      });
      return;
    }

    if (!isEditing) {
      setDraft(createDraft());
    }
  }, [isEditing, selectedTemplate]);

  if (isEditing && !selectedTemplate) {
    return (
      <main className="p-4 lg:p-6">
        <Empty className="min-h-[28rem] border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} />
            </EmptyMedia>
            <EmptyTitle>Template not found</EmptyTitle>
            <EmptyDescription>
              This template may have been removed from this browser.
            </EmptyDescription>
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

  function handleSave() {
    const name = draft.name.trim();

    if (!name) {
      return;
    }

    const now = new Date().toISOString();

    if (selectedTemplate) {
      templateCollection.update(selectedTemplate.id, (template) => {
        template.content = draft.content;
        template.name = name;
        template.updatedAt = now;
      });
      toast.success("Template updated");
      return;
    }

    const nextId = getNextTemplateId(templates);
    templateCollection.insert({
      id: nextId,
      content: draft.content,
      createdAt: now,
      name,
      updatedAt: now,
    });
    void navigate({
      to: "/templates/$templateId",
      params: { templateId: `${nextId}` },
    });
  }

  function handleDelete() {
    if (!selectedTemplate) {
      return;
    }

    templateCollection.delete(selectedTemplate.id);
    setIsDeleteDialogOpen(false);
    void navigate({ to: "/templates" });
  }

  return (
    <main className="grid w-full min-w-0 max-w-full grid-cols-[minmax(0,1fr)] gap-4 p-4">
      <section className="flex min-w-0 max-w-full items-center justify-between gap-3">
        <Breadcrumb>
          <BreadcrumbList>
            <BreadcrumbItem>
              <BreadcrumbLink render={<Link to="/templates" />}>Templates</BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
            <BreadcrumbItem>
              <BreadcrumbPage>
                {selectedTemplate?.name || (isEditing ? "Template" : "New")}
              </BreadcrumbPage>
            </BreadcrumbItem>
          </BreadcrumbList>
        </Breadcrumb>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            aria-label="Previous template"
            disabled={!previousTemplate}
            onClick={() => {
              if (previousTemplate) {
                void navigate({
                  to: "/templates/$templateId",
                  params: { templateId: `${previousTemplate.id}` },
                });
              }
            }}
            size="icon-sm"
            type="button"
            variant="outline"
          >
            <HugeiconsIcon icon={ArrowUp01Icon} />
          </Button>
          <Button
            aria-label="Next template"
            disabled={!nextTemplate}
            onClick={() => {
              if (nextTemplate) {
                void navigate({
                  to: "/templates/$templateId",
                  params: { templateId: `${nextTemplate.id}` },
                });
              }
            }}
            size="icon-sm"
            type="button"
            variant="outline"
          >
            <HugeiconsIcon icon={ArrowDown01Icon} />
          </Button>
          {selectedTemplate ? (
            <Button
              aria-label="Delete template"
              onClick={() => setIsDeleteDialogOpen(true)}
              size="icon-sm"
              type="button"
              variant="destructive"
            >
              <HugeiconsIcon icon={Delete02Icon} />
            </Button>
          ) : null}
          <Button disabled={!draft.name.trim()} onClick={handleSave} type="button">
            Save
          </Button>
        </div>
      </section>

      <section className="grid w-full min-w-0 max-w-full grid-cols-[minmax(0,1fr)] gap-4">
        <div className="ml-auto" dir="rtl">
          <InputGroup className="max-w-xl">
            <InputGroupAddon align="inline-start">
              <HugeiconsIcon icon={FileEditIcon} />
            </InputGroupAddon>
            <InputGroupInput
              aria-label="ٹیمپلیٹ کا نام"
              id="template-name"
              onChange={(event) =>
                setDraft((value) => ({
                  ...value,
                  name: event.target.value,
                }))
              }
              placeholder="ٹیمپلیٹ کا نام"
              value={draft.name}
            />
          </InputGroup>
        </div>

        <div className="grid w-full min-w-0 max-w-full grid-cols-[minmax(0,1fr)] [contain:inline-size]">
          <TemplateRichEditor
            aria-label="ٹیمپلیٹ کا متن"
            className="min-h-[42rem] rounded-none border-0 bg-transparent px-1 py-1 shadow-none focus-visible:ring-0"
            id="template-content"
            onChange={(content) =>
              setDraft((value) => ({
                ...value,
                content,
              }))
            }
            placeholder="ٹیمپلیٹ کا متن یہاں پیسٹ کریں..."
            value={draft.content}
          />
        </div>
        {placeholders.length ? (
          <div className="flex flex-wrap gap-2 py-3">
            {placeholders.map((placeholder) => (
              <Badge key={placeholder} variant="outline">
                {placeholder}
              </Badge>
            ))}
          </div>
        ) : null}
      </section>

      <AlertDialog open={isDeleteDialogOpen} onOpenChange={setIsDeleteDialogOpen}>
        <AlertDialogContent size="sm">
          <AlertDialogHeader>
            <AlertDialogTitle>Delete template</AlertDialogTitle>
            <AlertDialogDescription>
              This removes the template from this browser. FIR records and saved placeholder values
              are not deleted.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={handleDelete} type="button" variant="destructive">
              <HugeiconsIcon data-icon="inline-start" icon={Delete02Icon} />
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </main>
  );
}

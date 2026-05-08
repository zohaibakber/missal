import { useEffect, useMemo, useState } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { ClientOnly, Link, createFileRoute, useNavigate } from "@tanstack/react-router";
import {
  ArrowDown01Icon,
  ArrowUp01Icon,
  Delete02Icon,
  LegalDocument01Icon,
  SaveIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { firCollection, firPlaceholderValueCollection, templateCollection } from "#/db-collections";
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
import { TemplateRichEditor } from "#/components/template-rich-editor";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import { Skeleton } from "#/components/ui/skeleton";
import { Switch } from "#/components/ui/switch";
import { useToast } from "#/components/ui/toast";
import { buildTemplateValues, extractPlaceholders, renderTemplateHtml } from "#/lib/templates";

export const Route = createFileRoute("/$firId")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <ClientOnly fallback={<FirDetailSkeleton />}>
      <FirDetail />
    </ClientOnly>
  );
}

function FirDetail() {
  const { firId } = Route.useParams();
  const navigate = useNavigate();
  const toast = useToast();
  const numericFirId = Number(firId);
  const { data: firRecords } = useLiveQuery(firCollection);
  const { data: templates } = useLiveQuery(templateCollection);
  const { data: placeholderValues } = useLiveQuery(firPlaceholderValueCollection);
  const fir = firRecords.find((record) => record.id === numericFirId) ?? null;
  const sortedTemplates = useMemo(
    () => [...templates].sort((first, second) => first.id - second.id),
    [templates],
  );
  const firExtraValues = useMemo(
    () => placeholderValues.filter((value) => value.firId === numericFirId),
    [numericFirId, placeholderValues],
  );
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>("");
  const [documentDraft, setDocumentDraft] = useState("");
  const [isDocumentDirty, setIsDocumentDirty] = useState(false);
  const [arePlaceholderValuesVisible, setArePlaceholderValuesVisible] = useState(true);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);

  const selectedTemplate =
    templates.find((template) => `${template.id}` === selectedTemplateId) ?? null;
  const selectedTemplateIndex = selectedTemplate
    ? sortedTemplates.findIndex((template) => template.id === selectedTemplate.id)
    : -1;
  const previousTemplate =
    selectedTemplateIndex > 0 ? sortedTemplates[selectedTemplateIndex - 1] : null;
  const nextTemplate =
    selectedTemplateIndex >= 0 && selectedTemplateIndex < sortedTemplates.length - 1
      ? sortedTemplates[selectedTemplateIndex + 1]
      : null;
  const placeholders = useMemo(
    () => extractPlaceholders(selectedTemplate?.content ?? ""),
    [selectedTemplate],
  );
  const values = useMemo(() => {
    if (!fir) {
      return {};
    }

    const nextValues = buildTemplateValues({
      extraValues: firExtraValues,
      fir,
      placeholders,
    });

    return nextValues;
  }, [fir, firExtraValues, placeholders]);
  const renderedTemplateHtml = selectedTemplate
    ? arePlaceholderValuesVisible
      ? renderTemplateHtml(selectedTemplate.content, values)
      : selectedTemplate.content
    : "";

  useEffect(() => {
    const savedTemplate = fir?.templateId
      ? templates.find((template) => template.id === fir.templateId)
      : null;
    const firstTemplate = savedTemplate ?? templates[0];

    if (!selectedTemplateId && firstTemplate) {
      setSelectedTemplateId(`${firstTemplate.id}`);
    }
  }, [fir, selectedTemplateId, templates]);

  useEffect(() => {
    setIsDocumentDirty(false);
    setDocumentDraft(fir?.content ?? "");
  }, [numericFirId, fir?.content]);

  useEffect(() => {
    if (isDocumentDirty) {
      return;
    }

    setDocumentDraft(fir?.content || renderedTemplateHtml);
  }, [fir?.content, isDocumentDirty, renderedTemplateHtml]);

  if (!Number.isInteger(numericFirId) || !fir) {
    return (
      <main className="p-4 lg:p-6">
        <Empty className="min-h-[28rem] border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} />
            </EmptyMedia>
            <EmptyTitle>FIR not found</EmptyTitle>
            <EmptyDescription>This FIR may have been removed from this browser.</EmptyDescription>
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

  function handleSelectTemplate(value: string | null) {
    if (!value) {
      return;
    }

    const currentFir = fir;
    if (!currentFir) {
      return;
    }

    const nextTemplate = templates.find((template) => `${template.id}` === value);
    setSelectedTemplateId(value);

    if (nextTemplate) {
      const nextPlaceholders = extractPlaceholders(nextTemplate.content);
      const nextValues = buildTemplateValues({
        extraValues: firExtraValues,
        fir: currentFir,
        placeholders: nextPlaceholders,
      });

      setDocumentDraft(
        arePlaceholderValuesVisible
          ? renderTemplateHtml(nextTemplate.content, nextValues)
          : nextTemplate.content,
      );
      setIsDocumentDirty(true);
    }
  }

  function handleTogglePlaceholderValues(checked: boolean) {
    setArePlaceholderValuesVisible(checked);

    if (!selectedTemplate) {
      return;
    }

    setDocumentDraft(
      checked ? renderTemplateHtml(selectedTemplate.content, values) : selectedTemplate.content,
    );
    setIsDocumentDirty(true);
  }

  function handleUpdate() {
    if (!fir) {
      return;
    }

    firCollection.update(fir.id, (draft) => {
      draft.content = documentDraft;
      draft.templateId = selectedTemplate ? selectedTemplate.id : undefined;
    });
    setIsDocumentDirty(false);
    toast.success("FIR updated");
  }

  function handleDelete() {
    if (!fir) return;

    firCollection.delete(fir.id);

    for (const value of Array.from(firPlaceholderValueCollection.state.values())) {
      if (value.firId === fir.id) {
        firPlaceholderValueCollection.delete(value.id);
      }
    }

    setIsDeleteDialogOpen(false);
    void navigate({ to: "/" });
  }

  return (
    <main className="flex flex-col gap-4 p-4">
      <section className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div className="flex min-w-0 flex-col gap-2">
          <Breadcrumb>
            <BreadcrumbList>
              <BreadcrumbItem>
                <BreadcrumbLink render={<Link to="/" />}>Dataset</BreadcrumbLink>
              </BreadcrumbItem>
              <BreadcrumbSeparator />
              <BreadcrumbItem>
                <BreadcrumbPage>FIR {fir.fir_no}</BreadcrumbPage>
              </BreadcrumbItem>
            </BreadcrumbList>
          </Breadcrumb>
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-xl font-medium">FIR {fir.fir_no}</h1>
            <Badge variant="outline">{fir.status}</Badge>
          </div>
          <p className="line-clamp-1 text-sm text-muted-foreground">{fir.offence}</p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {selectedTemplate ? (
            <label className="flex h-8 items-center gap-2 px-2.5 text-sm">
              <span>{arePlaceholderValuesVisible ? "Values" : "Placeholders"}</span>
              <Switch
                aria-label="Switch between placeholder values and placeholders"
                checked={arePlaceholderValuesVisible}
                onCheckedChange={handleTogglePlaceholderValues}
              />
            </label>
          ) : null}
          {templates.length ? (
            <Select onValueChange={handleSelectTemplate} value={selectedTemplateId}>
              <SelectTrigger
                aria-label="Select template"
                className="w-full min-w-72 sm:w-auto sm:max-w-xl"
                dir="rtl"
              >
                <SelectValue placeholder="Select template">
                  {selectedTemplate?.name ?? "Select template"}
                </SelectValue>
              </SelectTrigger>
              <SelectContent dir="rtl">
                <SelectGroup>
                  {sortedTemplates.map((template) => (
                    <SelectItem key={template.id} value={`${template.id}`}>
                      {template.name}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          ) : null}
          <Button
            aria-label="Previous template"
            disabled={!previousTemplate}
            onClick={() => {
              if (previousTemplate) {
                handleSelectTemplate(`${previousTemplate.id}`);
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
                handleSelectTemplate(`${nextTemplate.id}`);
              }
            }}
            size="icon-sm"
            type="button"
            variant="outline"
          >
            <HugeiconsIcon icon={ArrowDown01Icon} />
          </Button>
          <Button
            disabled={!documentDraft.trim()}
            onClick={handleUpdate}
            type="button"
            variant="outline"
          >
            <HugeiconsIcon data-icon="inline-start" icon={SaveIcon} />
          </Button>
          <Button onClick={() => setIsDeleteDialogOpen(true)} type="button" variant="destructive">
            <HugeiconsIcon data-icon="inline-start" icon={Delete02Icon} />
          </Button>
        </div>
      </section>

      {selectedTemplate || documentDraft ? (
        <TemplateRichEditor
          aria-label="FIR content"
          className="min-h-[calc(100dvh-10rem)] rounded-none border-0 bg-transparent px-1 py-1 shadow-none focus-visible:ring-0"
          id="fir-content"
          onChange={(content) => {
            setDocumentDraft(content);
            setIsDocumentDirty(true);
          }}
          placeholder="FIR content"
          value={documentDraft}
        />
      ) : (
        <Empty className="min-h-[32rem] border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} />
            </EmptyMedia>
            <EmptyTitle>No template selected</EmptyTitle>
            <EmptyDescription>
              Create a template first, then return here to fill it for this FIR.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button nativeButton={false} render={<Link to="/templates" />} variant="outline">
              Open templates
            </Button>
          </EmptyContent>
        </Empty>
      )}
      <AlertDialog open={isDeleteDialogOpen} onOpenChange={setIsDeleteDialogOpen}>
        <AlertDialogContent size="sm">
          <AlertDialogHeader>
            <AlertDialogTitle>Delete FIR</AlertDialogTitle>
            <AlertDialogDescription>
              This removes FIR {fir.fir_no} and its saved template placeholder values from this
              browser.
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

function FirDetailSkeleton() {
  return (
    <main className="flex flex-col gap-4 p-4 lg:p-6">
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-2">
          <Skeleton className="h-8 w-24" />
          <Skeleton className="h-7 w-40" />
          <Skeleton className="h-4 w-64" />
        </div>
        <div className="flex gap-2">
          <Skeleton className="h-8 w-24" />
          <Skeleton className="h-8 w-20" />
        </div>
      </div>
      <div className="grid gap-4 xl:grid-cols-[1fr_minmax(20rem,26rem)]">
        <Skeleton className="h-[42rem]" />
        <Skeleton className="h-[42rem]" />
      </div>
    </main>
  );
}

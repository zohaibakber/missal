import { useEffect, useMemo, useState } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { ClientOnly, Link, createFileRoute, useNavigate } from "@tanstack/react-router";
import { toast } from "sonner";
import {
  ArrowDown01Icon,
  ArrowUp01Icon,
  Delete02Icon,
  LegalDocument01Icon,
  MoreVerticalIcon,
  PrinterIcon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  appSettingsCollection,
  firCollection,
  firPlaceholderValueCollection,
  getAppSettings,
  templateCollection,
} from "#/db-collections";
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
  DropdownMenu,
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
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import { InputGroup, InputGroupAddon, InputGroupInput } from "#/components/ui/input-group";
import { Skeleton } from "#/components/ui/skeleton";
import { buildTemplateValues, extractPlaceholders, renderTemplateHtml } from "#/lib/templates";
import { getFirStatusLabel } from "#/lib/fir";
import { buildSharedPlaceholderValues } from "#/lib/settings";

export const Route = createFileRoute("/$firId")({
  component: RouteComponent,
});

function escapeDocumentText(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function getPrintableFirHtml({ content, title }: { content: string; title: string }) {
  return `<!doctype html>
<html lang="ur" dir="rtl">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${escapeDocumentText(title)}</title>
    <style>
      @font-face {
        font-family: "Jameel Noori Nastaleeq";
        src: url("/Jameel%20Noori%20Nastaleeq.ttf") format("truetype");
        font-display: swap;
      }
      @page {
        size: A4;
        margin: 18mm;
      }
      * { box-sizing: border-box; }
      body {
        margin: 0;
        color: #111827;
        background: #ffffff;
        font-family: "Jameel Noori Nastaleeq", serif;
        font-size: 13pt;
        line-height: 2;
      }
      .fir-print-document {
        direction: rtl;
        color: inherit;
        font-family: inherit;
        font-size: inherit;
        line-height: inherit;
        text-align: right;
        unicode-bidi: plaintext;
        overflow-wrap: anywhere;
      }
      .fir-print-document > :first-child {
        margin-top: 0;
      }
      .fir-print-document > :last-child {
        margin-bottom: 0;
      }
      .fir-print-document p,
      .fir-print-document div,
      .fir-print-document li {
        text-align: inherit;
      }
      .fir-print-document p {
        margin: 0 0 0.5rem;
      }
      .fir-print-document h1,
      .fir-print-document h2,
      .fir-print-document h3,
      .fir-print-document h4,
      .fir-print-document h5,
      .fir-print-document h6 {
        margin: 0.75rem 0 0.35rem;
        font-weight: 700;
        line-height: 1.6;
      }
      .fir-print-document h1 { font-size: 1.75em; }
      .fir-print-document h2 { font-size: 1.5em; }
      .fir-print-document h3 { font-size: 1.25em; }
      .fir-print-document ul,
      .fir-print-document ol {
        margin: 0 1.5rem 0.5rem 0;
        padding: 0;
      }
      .fir-print-document blockquote {
        margin: 0 1rem 0.75rem 0;
        padding: 0.25rem 1rem 0.25rem 0;
        border-right: 4px solid #d1d5db;
      }
      .fir-print-document .table,
      .fir-print-document figure.table {
        display: table;
        margin: 0.75rem auto;
        max-width: 100%;
      }
      .fir-print-document figure.table[style*="width"] {
        display: table;
      }
      .fir-print-document table {
        max-width: 100%;
        border-collapse: collapse;
        border-spacing: 0;
      }
      .fir-print-document figure.table table {
        width: 100%;
      }
      .fir-print-document td,
      .fir-print-document th {
        border: 1px solid #d1d5db;
        padding: 0.25rem 0.35rem;
        min-width: 0;
        overflow-wrap: anywhere;
        word-break: break-word;
        white-space: normal;
      }
      .fir-print-document figure.image {
        display: table;
        clear: both;
        margin: 0.75rem auto;
        text-align: center;
      }
      .fir-print-document figure.image.image-style-side {
        float: left;
        max-width: 50%;
        margin-right: 1rem;
      }
      .fir-print-document figure.image.image-style-align-left {
        float: left;
        margin-right: 1rem;
      }
      .fir-print-document figure.image.image-style-align-right {
        float: right;
        margin-left: 1rem;
      }
      .fir-print-document figure.image.image-style-align-center {
        margin-right: auto;
        margin-left: auto;
      }
      .fir-print-document img {
        display: block;
        max-width: 100%;
        height: auto;
      }
      .fir-print-document figcaption {
        caption-side: bottom;
        color: #4b5563;
        display: table-caption;
        font-size: 0.85em;
        outline-offset: -1px;
        padding: 0.35rem;
        text-align: center;
        word-break: break-word;
      }
      .fir-print-document .page-break {
        clear: both;
        page-break-after: always;
        break-after: page;
      }
      .fir-print-document hr {
        border: 0;
        border-top: 1px solid #d1d5db;
        margin: 1rem 0;
      }
      .fir-print-document [data-placeholder="true"] {
        color: #1d4ed8;
        font-weight: 600;
      }
      .fir-print-document::after {
        clear: both;
        content: "";
        display: block;
      }
      @media print {
        body {
          print-color-adjust: exact;
          -webkit-print-color-adjust: exact;
        }
        .fir-print-document figure,
        .fir-print-document table,
        .fir-print-document img {
          break-inside: avoid;
          page-break-inside: avoid;
        }
      }
    </style>
  </head>
  <body>
    <main class="ck-content fir-print-document">${content}</main>
  </body>
</html>`;
}

function printHtmlDocument(html: string, title: string) {
  const existingFrame = document.getElementById("fir-print-frame");
  existingFrame?.remove();

  const frame = document.createElement("iframe");
  frame.id = "fir-print-frame";
  frame.title = title;
  frame.setAttribute("aria-hidden", "true");
  frame.style.position = "fixed";
  frame.style.inset = "0";
  frame.style.width = "0";
  frame.style.height = "0";
  frame.style.border = "0";
  frame.style.visibility = "hidden";

  document.body.append(frame);

  const frameWindow = frame.contentWindow;
  const frameDocument = frame.contentDocument ?? frameWindow?.document;

  if (!frameWindow || !frameDocument) {
    frame.remove();
    toast.error("Unable to prepare print view");
    return;
  }

  const cleanup = () => {
    window.setTimeout(() => frame.remove(), 500);
  };

  frameWindow.addEventListener("afterprint", cleanup, { once: true });
  frameDocument.open();
  frameDocument.write(html);
  frameDocument.close();

  window.setTimeout(() => {
    frameWindow.focus();
    frameWindow.print();
  }, 250);
}

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
  const numericFirId = Number(firId);
  const { data: firRecords } = useLiveQuery(firCollection);
  const { data: templates } = useLiveQuery(templateCollection);
  const { data: placeholderValues } = useLiveQuery(firPlaceholderValueCollection);
  const { data: settingsRecords } = useLiveQuery(appSettingsCollection);
  const fir = firRecords.find((record) => record.id === numericFirId) ?? null;
  const appSettings = getAppSettings(settingsRecords);
  const sharedPlaceholderValues = useMemo(
    () => buildSharedPlaceholderValues(appSettings.sharedPlaceholders),
    [appSettings.sharedPlaceholders],
  );
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
  const [templateSearch, setTemplateSearch] = useState("");
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
  const filteredTemplates = useMemo(() => {
    const query = templateSearch.trim().toLocaleLowerCase();

    if (!query) {
      return sortedTemplates;
    }

    return sortedTemplates.filter((template) => template.name.toLocaleLowerCase().includes(query));
  }, [sortedTemplates, templateSearch]);
  const documentSourceHtml = fir?.content || selectedTemplate?.content || "";
  const placeholders = useMemo(() => extractPlaceholders(documentSourceHtml), [documentSourceHtml]);
  const values = useMemo(() => {
    if (!fir) {
      return {};
    }

    const nextValues = buildTemplateValues({
      extraValues: firExtraValues,
      fir,
      placeholders,
      sharedValues: sharedPlaceholderValues,
    });

    return nextValues;
  }, [fir, firExtraValues, placeholders, sharedPlaceholderValues]);
  const renderedDocumentHtml = documentSourceHtml
    ? arePlaceholderValuesVisible
      ? renderTemplateHtml(documentSourceHtml, values)
      : documentSourceHtml
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

    setDocumentDraft(renderedDocumentHtml);
  }, [isDocumentDirty, renderedDocumentHtml]);

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
        sharedValues: sharedPlaceholderValues,
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

    const sourceHtml = fir?.content || selectedTemplate?.content || "";

    if (!sourceHtml) {
      return;
    }

    setDocumentDraft(checked ? renderTemplateHtml(sourceHtml, values) : sourceHtml);
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

  function handlePrintDocument() {
    if (!fir) {
      return;
    }

    if (!documentDraft.trim()) {
      toast.warning("Nothing to print");
      return;
    }

    const title = `FIR ${fir.fir_no}`;
    printHtmlDocument(
      getPrintableFirHtml({
        content: documentDraft,
        title,
      }),
      title,
    );
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
            <Badge variant="outline">{getFirStatusLabel(fir.status)}</Badge>
          </div>
          <p className="line-clamp-1 text-sm text-muted-foreground">{fir.offence}</p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
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
              <SelectContent dir="rtl" alignItemWithTrigger={false} className="w-80">
                <div className="p-1">
                  <InputGroup>
                    <InputGroupAddon align="inline-start">
                      <HugeiconsIcon
                        aria-hidden="true"
                        className="text-muted-foreground"
                        icon={Search01Icon}
                        strokeWidth={2}
                      />
                    </InputGroupAddon>
                    <InputGroupInput
                      aria-label="Search templates"
                      autoComplete="off"
                      onChange={(event) => setTemplateSearch(event.target.value)}
                      onKeyDown={(event) => event.stopPropagation()}
                      placeholder="تلاش کریں"
                      value={templateSearch}
                    />
                  </InputGroup>
                </div>
                <SelectGroup>
                  {filteredTemplates.length ? (
                    filteredTemplates.map((template) => (
                      <SelectItem key={template.id} value={`${template.id}`}>
                        {template.name}
                      </SelectItem>
                    ))
                  ) : (
                    <div className="px-2 py-2 text-sm text-muted-foreground">
                      کوئی ٹیمپلیٹ نہیں ملا
                    </div>
                  )}
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

          <DropdownMenu>
            <DropdownMenuTrigger render={<Button type="button" variant="outline" size="icon-sm" />}>
              <HugeiconsIcon icon={MoreVerticalIcon} strokeWidth={2} />
              <span className="sr-only">Open FIR actions</span>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-48">
              {selectedTemplate ? (
                <>
                  <DropdownMenuCheckboxItem
                    checked={arePlaceholderValuesVisible}
                    onCheckedChange={handleTogglePlaceholderValues}
                  >
                    Show values
                  </DropdownMenuCheckboxItem>
                  <DropdownMenuSeparator />
                </>
              ) : null}
              <DropdownMenuItem disabled={!documentDraft.trim()} onClick={handlePrintDocument}>
                <HugeiconsIcon icon={PrinterIcon} strokeWidth={2} />
                Print
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => setIsDeleteDialogOpen(true)} variant="destructive">
                <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                Delete
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button disabled={!documentDraft.trim()} onClick={handleUpdate} type="button">
            Update
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

import { useEffect, useMemo, useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Cause, Exit, Match, Option } from "effect";
import { Link, createFileRoute, useNavigate } from "@tanstack/react-router";
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
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemTitle,
} from "#/components/ui/item";
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
import { formatDate } from "#/lib/date";
import { FirDocumentUpdateInput, getFirStatusLabel } from "#/lib/fir";
import { parseFirId, type FirId } from "#/lib/ids";
import { indexPlaceholders } from "#/lib/placeholder";
import { buildSharedPlaceholderValues } from "#/lib/settings";
import { atoms } from "#/state/atoms";

export const Route = createFileRoute("/$firId_/edit")({
  component: RouteComponent,
  validateSearch: (search: Record<string, unknown>) => ({
    templateId: typeof search.templateId === "string" ? search.templateId : undefined,
  }),
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
        src: url("./Jameel%20Noori%20Nastaleeq.ttf") format("truetype");
        font-display: swap;
      }
      @page {
        size: A4;
        margin: 18mm;
      }
      * { box-sizing: border-box; }
      html {
        background: #ffffff;
      }
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
        display: block;
        width: 100%;
        max-width: 100%;
        color: inherit;
        font-family: inherit;
        font-size: inherit;
        line-height: inherit;
        text-align: right;
        unicode-bidi: isolate;
        overflow-wrap: break-word;
        word-break: normal;
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
        width: 100%;
        max-width: 100%;
        border-collapse: collapse;
        border-spacing: 0;
        table-layout: fixed;
      }
      .fir-print-document figure.table table {
        width: 100%;
      }
      .fir-print-document td,
      .fir-print-document th {
        border: 1px solid #d1d5db;
        padding: 0.25rem 0.35rem;
        min-width: 0;
        overflow-wrap: break-word;
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
        .fir-print-document {
          width: auto;
          max-width: none;
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
  frame.style.insetBlockStart = "0";
  frame.style.insetInlineStart = "-10000px";
  frame.style.width = "210mm";
  frame.style.height = "297mm";
  frame.style.border = "0";
  frame.style.opacity = "0";
  frame.style.pointerEvents = "none";

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

  const printFrame = () => {
    frameWindow.focus();
    frameWindow.print();
  };

  const waitForImages = Promise.all(
    Array.from(frameDocument.images).map((image) => {
      if (image.complete) {
        return Promise.resolve();
      }

      return new Promise<void>((resolve) => {
        image.addEventListener("load", () => resolve(), { once: true });
        image.addEventListener("error", () => resolve(), { once: true });
      });
    }),
  );
  const waitForFonts = "fonts" in frameDocument ? frameDocument.fonts.ready : Promise.resolve();

  Promise.all([waitForFonts, waitForImages])
    .catch(() => undefined)
    .finally(() => {
      window.setTimeout(printFrame, 50);
    });
}

function RouteComponent() {
  const { firId } = Route.useParams();
  const id = parseFirId(firId);

  if (!id) {
    return <FirNotFound />;
  }

  return <FirDetail firId={id} />;
}

function FirNotFound() {
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

function getTemplateSummary(content: string) {
  return content
    .replace(/<[^>]*>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function FirDetail({ firId }: { firId: FirId }) {
  const { templateId } = Route.useSearch();
  const navigate = useNavigate();
  const editor = useAtomValue(atoms.firEditorAtom(firId));
  const updateDocument = useAtomSet(atoms.updateFirDocumentAtom, { mode: "promiseExit" });
  const removeFir = useAtomSet(atoms.removeFirAtom, { mode: "promiseExit" });
  const editorValue = AsyncResult.isSuccess(editor) ? editor.value : undefined;
  const fir = editorValue?.fir ?? null;
  const templates = editorValue?.templates ?? [];
  const firExtraValues = editorValue?.values ?? [];
  const placeholderIndex = editorValue
    ? indexPlaceholders(editorValue.placeholders)
    : indexPlaceholders([]);
  const appSettings = editorValue?.settings;
  const sharedPlaceholderValues = useMemo(
    () => buildSharedPlaceholderValues(appSettings?.sharedPlaceholders ?? {}),
    [appSettings?.sharedPlaceholders],
  );
  const sortedTemplates = useMemo(
    () => [...templates].sort((first, second) => first.id - second.id),
    [templates],
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
  const placeholders = useMemo(
    () => extractPlaceholders(documentSourceHtml, placeholderIndex),
    [documentSourceHtml, placeholderIndex],
  );
  const values = useMemo(() => {
    if (!fir) {
      return {};
    }

    const nextValues = buildTemplateValues({
      catalog: placeholderIndex,
      extraValues: firExtraValues,
      fir,
      placeholders,
      sharedValues: sharedPlaceholderValues,
    });

    return nextValues;
  }, [fir, firExtraValues, placeholderIndex, placeholders, sharedPlaceholderValues]);
  const renderedDocumentHtml = documentSourceHtml
    ? arePlaceholderValuesVisible
      ? renderTemplateHtml(documentSourceHtml, values, placeholderIndex)
      : documentSourceHtml
    : "";

  useEffect(() => {
    if (!templateId || selectedTemplateId === templateId) {
      return;
    }

    const requestedTemplate = templates.find((template) => `${template.id}` === templateId);

    if (requestedTemplate) {
      setSelectedTemplateId(templateId);
    }
  }, [selectedTemplateId, templateId, templates]);

  useEffect(() => {
    setIsDocumentDirty(false);
    setDocumentDraft(fir?.content ?? "");
  }, [firId, fir?.content]);

  useEffect(() => {
    if (isDocumentDirty) {
      return;
    }

    setDocumentDraft(renderedDocumentHtml);
  }, [isDocumentDirty, renderedDocumentHtml]);

  if (AsyncResult.isInitial(editor) || AsyncResult.isWaiting(editor)) {
    return <FirDetailSkeleton />;
  }

  if (AsyncResult.isFailure(editor)) {
    return Option.match(Cause.findErrorOption(editor.cause), {
      onNone: () => <FirDetailSkeleton />,
      onSome: (error) =>
        Match.value(error).pipe(
          Match.tag("EntityNotFound", () => <FirNotFound />),
          Match.orElse(() => <FirDetailSkeleton />),
        ),
    });
  }

  if (!fir) {
    return <FirNotFound />;
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
    void navigate({
      replace: true,
      search: { templateId: value },
      to: "/$firId/edit",
      params: { firId: `${currentFir.id}` },
    });

    if (nextTemplate) {
      const nextPlaceholders = extractPlaceholders(nextTemplate.content, placeholderIndex);
      const nextValues = buildTemplateValues({
        catalog: placeholderIndex,
        extraValues: firExtraValues,
        fir: currentFir,
        placeholders: nextPlaceholders,
        sharedValues: sharedPlaceholderValues,
      });

      setDocumentDraft(
        arePlaceholderValuesVisible
          ? renderTemplateHtml(nextTemplate.content, nextValues, placeholderIndex)
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

    setDocumentDraft(
      checked ? renderTemplateHtml(sourceHtml, values, placeholderIndex) : sourceHtml,
    );
    setIsDocumentDirty(true);
  }

  async function handleUpdate() {
    if (!fir) {
      return;
    }

    const exit = await updateDocument(
      new FirDocumentUpdateInput({
        id: fir.id,
        content: documentDraft,
        ...(selectedTemplate ? { templateId: selectedTemplate.id } : {}),
      }),
    );

    if (Exit.isFailure(exit)) {
      toast.error(
        Option.match(Cause.findErrorOption(exit.cause), {
          onNone: () => "Something went wrong while saving",
          onSome: (error) =>
            Match.valueTags(error, {
              EntityNotFound: ({ entity }) => `${entity} not found`,
              EntityConflict: ({ field }) => `${field} is already in use`,
              StorageError: ({ message }) => message,
            }),
        }),
      );
      return;
    }

    setIsDocumentDirty(false);
    toast.success("FIR updated");
  }

  async function handleDelete() {
    if (!fir) {
      return;
    }

    const exit = await removeFir(fir.id);

    if (Exit.isFailure(exit)) {
      toast.error(
        Option.match(Cause.findErrorOption(exit.cause), {
          onNone: () => "Something went wrong while saving",
          onSome: (error) =>
            Match.valueTags(error, {
              EntityNotFound: ({ entity }) => `${entity} not found`,
              EntityConflict: ({ field }) => `${field} is already in use`,
              StorageError: ({ message }) => message,
            }),
        }),
      );
      return;
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
    const printPlaceholders = extractPlaceholders(documentDraft, placeholderIndex);
    const printValues = buildTemplateValues({
      catalog: placeholderIndex,
      extraValues: firExtraValues,
      fir,
      placeholders: printPlaceholders,
      sharedValues: sharedPlaceholderValues,
    });

    printHtmlDocument(
      getPrintableFirHtml({
        content: renderTemplateHtml(documentDraft, printValues, placeholderIndex),
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
                <BreadcrumbLink render={<Link to="/$firId" params={{ firId: `${fir.id}` }} />}>
                  FIR {fir.fir_no}
                </BreadcrumbLink>
              </BreadcrumbItem>
              <BreadcrumbSeparator />
              <BreadcrumbItem>
                <BreadcrumbPage>Edit</BreadcrumbPage>
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
        <>
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
        </>
      ) : templates.length ? (
        <section className="flex flex-col gap-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex min-w-0 flex-col gap-1">
              <h2 className="text-base font-medium">Select template</h2>
              <p className="text-sm text-muted-foreground">
                Choose a template to fill with FIR {fir.fir_no} values.
              </p>
            </div>
            <div className="w-full sm:max-w-sm" dir="rtl">
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
                  placeholder="تلاش کریں"
                  value={templateSearch}
                />
              </InputGroup>
            </div>
          </div>

          {filteredTemplates.length ? (
            <ItemGroup className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
              {filteredTemplates.map((template) => {
                const templatePlaceholders = extractPlaceholders(
                  template.content,
                  placeholderIndex,
                );
                const summary = getTemplateSummary(template.content);

                return (
                  <Item
                    key={template.id}
                    className="min-h-36 cursor-pointer items-start bg-muted text-start"
                    dir="rtl"
                    onClick={() => handleSelectTemplate(`${template.id}`)}
                    render={<button type="button" />}
                    variant="outline"
                  >
                    <ItemContent className="min-w-0" lang="ur">
                      <ItemTitle>{template.name}</ItemTitle>
                      <ItemDescription className="line-clamp-1 text-xs">
                        Updated {formatDate(template.updatedAt)}
                      </ItemDescription>
                      <ItemDescription className="line-clamp-3">
                        {summary || "No content yet."}
                      </ItemDescription>
                    </ItemContent>
                    <ItemActions className="self-start" dir="ltr">
                      <Badge variant="outline">{templatePlaceholders.length}</Badge>
                    </ItemActions>
                  </Item>
                );
              })}
            </ItemGroup>
          ) : (
            <Empty className="min-h-[24rem] border">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <HugeiconsIcon icon={LegalDocument01Icon} />
                </EmptyMedia>
                <EmptyTitle>No templates found</EmptyTitle>
                <EmptyDescription>Clear search to see saved templates.</EmptyDescription>
              </EmptyHeader>
            </Empty>
          )}
        </section>
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
              This removes FIR {fir.fir_no} and its saved template placeholder values.
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

import { useEffect, useMemo, useState } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { ClientOnly, Link, createFileRoute, useNavigate } from "@tanstack/react-router";
import {
  ArrowDown01Icon,
  ArrowUp01Icon,
  CheckmarkCircle02Icon,
  Copy01Icon,
  LegalDocument01Icon,
  SaveIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  firCollection,
  firPlaceholderValueCollection,
  templateCollection,
  upsertFirPlaceholderValue,
} from "#/db-collections";
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
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "#/components/ui/card";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import { Field, FieldGroup, FieldLabel } from "#/components/ui/field";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import { Separator } from "#/components/ui/separator";
import { Skeleton } from "#/components/ui/skeleton";
import { Textarea } from "#/components/ui/textarea";
import {
  buildTemplateValues,
  extractPlaceholders,
  getCorePlaceholderValue,
  getMissingPlaceholders,
  renderTemplate,
  renderTemplateHtml,
} from "#/lib/templates";

export const Route = createFileRoute("/dataset_/$firId")({
  component: RouteComponent,
});

function getPlainTextFromHtml(html: string) {
  const container = document.createElement("div");
  container.innerHTML = html;
  return container.innerText;
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
  const fir = firRecords.find((record) => record.id === numericFirId) ?? null;
  const sortedFirRecords = useMemo(
    () => [...firRecords].sort((first, second) => first.id - second.id),
    [firRecords],
  );
  const selectedFirIndex = fir ? sortedFirRecords.findIndex((record) => record.id === fir.id) : -1;
  const previousFir = selectedFirIndex > 0 ? sortedFirRecords[selectedFirIndex - 1] : null;
  const nextFir =
    selectedFirIndex >= 0 && selectedFirIndex < sortedFirRecords.length - 1
      ? sortedFirRecords[selectedFirIndex + 1]
      : null;
  const firExtraValues = useMemo(
    () => placeholderValues.filter((value) => value.firId === numericFirId),
    [numericFirId, placeholderValues],
  );
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>("");
  const [extraDraft, setExtraDraft] = useState<Record<string, string>>({});
  const [copied, setCopied] = useState(false);

  const selectedTemplate =
    templates.find((template) => `${template.id}` === selectedTemplateId) ?? null;
  const placeholders = useMemo(
    () => extractPlaceholders(selectedTemplate?.content ?? ""),
    [selectedTemplate],
  );
  const autoFilledPlaceholders = useMemo(() => {
    if (!fir) {
      return [];
    }

    return placeholders.filter(
      (placeholder) => getCorePlaceholderValue(placeholder, fir) !== undefined,
    );
  }, [fir, placeholders]);
  const extraPlaceholders = useMemo(() => {
    if (!fir) {
      return [];
    }

    return placeholders.filter(
      (placeholder) => getCorePlaceholderValue(placeholder, fir) === undefined,
    );
  }, [fir, placeholders]);
  const values = useMemo(() => {
    if (!fir) {
      return {};
    }

    const nextValues = buildTemplateValues({
      extraValues: firExtraValues,
      fir,
      placeholders,
    });

    for (const placeholder of extraPlaceholders) {
      nextValues[placeholder] = extraDraft[placeholder] ?? nextValues[placeholder] ?? "";
    }

    return nextValues;
  }, [extraDraft, extraPlaceholders, fir, firExtraValues, placeholders]);
  const renderedTemplate = selectedTemplate ? renderTemplate(selectedTemplate.content, values) : "";
  const renderedTemplateHtml = selectedTemplate
    ? renderTemplateHtml(selectedTemplate.content, values)
    : "";
  const missingPlaceholders = getMissingPlaceholders(placeholders, values);

  useEffect(() => {
    const firstTemplate = templates[0];

    if (!selectedTemplateId && firstTemplate) {
      setSelectedTemplateId(`${firstTemplate.id}`);
    }
  }, [selectedTemplateId, templates]);

  useEffect(() => {
    const nextDraft: Record<string, string> = {};

    for (const value of firExtraValues) {
      nextDraft[value.placeholder] = value.value;
    }

    setExtraDraft(nextDraft);
  }, [numericFirId, selectedTemplateId, firExtraValues]);

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
            <Button render={<Link to="/dataset" />} variant="outline">
              Back to dataset
            </Button>
          </EmptyContent>
        </Empty>
      </main>
    );
  }

  function handleSaveExtraValues() {
    for (const placeholder of extraPlaceholders) {
      upsertFirPlaceholderValue({
        firId: numericFirId,
        placeholder,
        value: extraDraft[placeholder] ?? "",
      });
    }
  }

  async function handleCopy() {
    const plainText = getPlainTextFromHtml(renderedTemplateHtml);

    if ("ClipboardItem" in window && navigator.clipboard.write) {
      await navigator.clipboard.write([
        new ClipboardItem({
          "text/html": new Blob([renderedTemplateHtml], { type: "text/html" }),
          "text/plain": new Blob([plainText], { type: "text/plain" }),
        }),
      ]);
    } else {
      await navigator.clipboard.writeText(plainText || renderedTemplate);
    }

    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  }

  return (
    <main className="flex flex-col gap-4 p-4">
      <section className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div className="flex min-w-0 flex-col gap-2">
          <Breadcrumb>
            <BreadcrumbList>
              <BreadcrumbItem>
                <BreadcrumbLink render={<Link to="/dataset" />}>Dataset</BreadcrumbLink>
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
        <div className="flex items-center gap-2">
          <Button
            aria-label="Previous FIR"
            disabled={!previousFir}
            onClick={() => {
              if (previousFir) {
                void navigate({
                  to: "/dataset/$firId",
                  params: { firId: `${previousFir.id}` },
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
            aria-label="Next FIR"
            disabled={!nextFir}
            onClick={() => {
              if (nextFir) {
                void navigate({
                  to: "/dataset/$firId",
                  params: { firId: `${nextFir.id}` },
                });
              }
            }}
            size="icon-sm"
            type="button"
            variant="outline"
          >
            <HugeiconsIcon icon={ArrowDown01Icon} />
          </Button>
          <Button
            disabled={!selectedTemplate}
            onClick={handleSaveExtraValues}
            type="button"
            variant="outline"
          >
            <HugeiconsIcon data-icon="inline-start" icon={SaveIcon} />
            Save values
          </Button>
          <Button disabled={!selectedTemplate} onClick={() => void handleCopy()} type="button">
            <HugeiconsIcon
              data-icon="inline-start"
              icon={copied ? CheckmarkCircle02Icon : Copy01Icon}
            />
            {copied ? "Copied" : "Copy"}
          </Button>
        </div>
      </section>

      <div className="grid gap-4 xl:grid-cols-[1fr_minmax(20rem,26rem)]">
        <Card className="gap-0 py-0">
          <CardHeader className="border-b py-4">
            <CardAction>
              {templates.length ? (
                <Select
                  onValueChange={(value) => {
                    if (value) {
                      setSelectedTemplateId(value);
                    }
                  }}
                  value={selectedTemplateId}
                >
                  <SelectTrigger aria-label="Select template" className="w-full sm:max-w-md">
                    <SelectValue placeholder="Select template" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {templates.map((template) => (
                        <SelectItem key={template.id} value={`${template.id}`}>
                          {template.name}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              ) : null}
            </CardAction>
          </CardHeader>
          <CardContent className="flex flex-col gap-4 p-4">
            {selectedTemplate ? (
              <>
                {missingPlaceholders.length ? (
                  <div className="flex flex-wrap gap-2">
                    {missingPlaceholders.map((placeholder) => (
                      <Badge key={placeholder} variant="destructive">
                        {placeholder}
                      </Badge>
                    ))}
                  </div>
                ) : null}
                <div
                  className="min-h-[32rem] overflow-auto rounded-lg border bg-background p-4 text-right leading-8 [&_*]:max-w-full [&_table]:w-full [&_table]:border-collapse [&_td]:border [&_td]:border-border [&_td]:p-1 [&_th]:border [&_th]:border-border [&_th]:p-1"
                  dangerouslySetInnerHTML={{ __html: renderedTemplateHtml }}
                  dir="rtl"
                  lang="ur"
                />
              </>
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
                  <Button render={<Link to="/templates" />} variant="outline">
                    Open templates
                  </Button>
                </EmptyContent>
              </Empty>
            )}
          </CardContent>
        </Card>

        <Card className="gap-0 py-0">
          <CardHeader className="border-b py-4">
            <CardTitle>Placeholders</CardTitle>
            <CardDescription>
              Core FIR fields fill automatically. Add values for template-only placeholders.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-5 p-4">
            <div className="flex flex-col gap-2">
              <p className="text-sm font-medium">Auto-filled</p>
              {autoFilledPlaceholders.length ? (
                <div className="flex flex-col gap-2">
                  {autoFilledPlaceholders.map((placeholder) => (
                    <div key={placeholder} className="rounded-lg border bg-muted/40 p-3">
                      <p className="text-xs text-muted-foreground">{placeholder}</p>
                      <p className="mt-1 line-clamp-2 text-sm">{values[placeholder]}</p>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-sm text-muted-foreground">
                  No selected template placeholders match FIR fields yet.
                </p>
              )}
            </div>

            <Separator />

            <FieldGroup>
              <p className="text-sm font-medium">Needs input</p>
              {extraPlaceholders.length ? (
                extraPlaceholders.map((placeholder) => (
                  <Field key={placeholder}>
                    <FieldLabel htmlFor={`placeholder-${placeholder}`}>{placeholder}</FieldLabel>
                    <Textarea
                      id={`placeholder-${placeholder}`}
                      className="min-h-20 resize-y"
                      dir="rtl"
                      lang="ur"
                      onChange={(event) =>
                        setExtraDraft((value) => ({
                          ...value,
                          [placeholder]: event.target.value,
                        }))
                      }
                      value={extraDraft[placeholder] ?? ""}
                    />
                  </Field>
                ))
              ) : (
                <p className="text-sm text-muted-foreground">
                  This template has no extra placeholders.
                </p>
              )}
            </FieldGroup>
          </CardContent>
        </Card>
      </div>
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

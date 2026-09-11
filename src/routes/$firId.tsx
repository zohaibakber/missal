import { useMemo, useState } from "react";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Link, createFileRoute } from "@tanstack/react-router";
import { LegalDocument01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
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
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemTitle,
} from "#/components/ui/item";
import { Skeleton } from "#/components/ui/skeleton";
import { formatDate } from "#/lib/date";
import { getFirStatusLabel } from "#/lib/fir";
import { parseFirId, type FirId } from "#/lib/ids";
import { indexPlaceholders } from "#/lib/placeholder";
import { extractPlaceholders } from "#/lib/templates";
import { atoms } from "#/state/atoms";

export const Route = createFileRoute("/$firId")({
  component: RouteComponent,
});

function RouteComponent() {
  const { firId } = Route.useParams();
  const id = parseFirId(firId);

  if (!id) {
    return <FirNotFound />;
  }

  return <FirTemplates firId={id} />;
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

function FirTemplates({ firId }: { firId: FirId }) {
  const firResult = useAtomValue(atoms.firByIdAtom(firId));
  const templatesResult = useAtomValue(atoms.templatesAtom);
  const placeholderIndexResult = useAtomValue(atoms.placeholderIndexAtom);
  const [search, setSearch] = useState("");
  const fir = AsyncResult.isSuccess(firResult) ? (firResult.value ?? null) : null;
  const templates = AsyncResult.isSuccess(templatesResult) ? templatesResult.value : [];
  const placeholderIndex = AsyncResult.isSuccess(placeholderIndexResult)
    ? placeholderIndexResult.value
    : indexPlaceholders([]);
  const sortedTemplates = useMemo(
    () => [...templates].sort((first, second) => first.id - second.id),
    [templates],
  );
  const filteredTemplates = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();

    if (!query) {
      return sortedTemplates;
    }

    return sortedTemplates.filter((template) => {
      const summary = getTemplateSummary(template.content).toLocaleLowerCase();

      return template.name.toLocaleLowerCase().includes(query) || summary.includes(query);
    });
  }, [search, sortedTemplates]);

  if (
    AsyncResult.isInitial(firResult) ||
    AsyncResult.isWaiting(firResult) ||
    AsyncResult.isInitial(templatesResult) ||
    AsyncResult.isWaiting(templatesResult)
  ) {
    return <FirTemplatesSkeleton />;
  }

  if (!fir) {
    return <FirNotFound />;
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
              onChange={(event) => setSearch(event.target.value)}
              placeholder="تلاش کریں"
              value={search}
            />
          </InputGroup>
        </div>
      </section>

      {templates.length ? (
        filteredTemplates.length ? (
          <ItemGroup className="grid gap-3 md:grid-cols-2 xl:grid-cols-3" dir="rtl">
            {filteredTemplates.map((template) => {
              const placeholders = extractPlaceholders(template.content, placeholderIndex);
              const summary = getTemplateSummary(template.content);

              return (
                <Item
                  key={template.id}
                  className="min-h-36 items-start bg-muted"
                  dir="rtl"
                  render={
                    <Link
                      to="/$firId/edit"
                      params={{ firId: `${fir.id}` }}
                      search={{ templateId: `${template.id}` }}
                    />
                  }
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
                    <Badge variant="outline">{placeholders.length}</Badge>
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
        )
      ) : (
        <Empty className="min-h-[32rem] border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} />
            </EmptyMedia>
            <EmptyTitle>No templates yet</EmptyTitle>
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
    </main>
  );
}

function FirTemplatesSkeleton() {
  return (
    <main className="flex flex-col gap-4 p-4 lg:p-6">
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-2">
          <Skeleton className="h-8 w-24" />
          <Skeleton className="h-7 w-40" />
          <Skeleton className="h-4 w-64" />
        </div>
        <Skeleton className="h-8 w-64" />
      </div>
      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        {Array.from({ length: 6 }, (_, index) => (
          <Skeleton key={index} className="h-36" />
        ))}
      </div>
    </main>
  );
}

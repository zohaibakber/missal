import { useMemo, useState } from "react";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Link, createFileRoute } from "@tanstack/react-router";
import { Add01Icon, LegalDocument01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Badge } from "#/components/ui/badge";
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
import { indexPlaceholders } from "#/lib/placeholder";
import { formatDate } from "#/lib/date";
import { extractPlaceholders } from "#/lib/templates";
import { atoms } from "#/state/atoms";

export const Route = createFileRoute("/templates")({
  component: RouteComponent,
});

function RouteComponent() {
  const templates = useAtomValue(atoms.templatesAtom);

  if (AsyncResult.isInitial(templates) || AsyncResult.isWaiting(templates)) {
    return <TemplatesSkeleton />;
  }

  return <TemplateList />;
}

function getTemplateSummary(content: string) {
  return content
    .replace(/<[^>]*>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function TemplateList() {
  const templatesResult = useAtomValue(atoms.templatesAtom);
  const placeholderIndexResult = useAtomValue(atoms.placeholderIndexAtom);
  const templates = AsyncResult.isSuccess(templatesResult) ? templatesResult.value : [];
  const placeholderIndex = AsyncResult.isSuccess(placeholderIndexResult)
    ? placeholderIndexResult.value
    : indexPlaceholders([]);
  const [search, setSearch] = useState("");
  const filteredTemplates = useMemo(() => {
    const query = search.trim().toLowerCase();

    if (!query) {
      return templates;
    }

    return templates.filter(
      (template) =>
        template.name.toLowerCase().includes(query) ||
        getTemplateSummary(template.content).toLowerCase().includes(query),
    );
  }, [search, templates]);

  return (
    <main className="flex flex-col gap-4 p-4">
      <section className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-col gap-1">
          <h1 className="text-xl font-medium">Templates</h1>
        </div>
        <Button nativeButton={false} render={<Link to="/templates/new" />}>
          New template
        </Button>
      </section>
      <div className="ml-auto" dir="rtl">
        <InputGroup className="max-w-sm">
          <InputGroupAddon align="inline-start">
            <HugeiconsIcon icon={Search01Icon} />
          </InputGroupAddon>
          <InputGroupInput
            aria-label="ٹیمپلیٹس تلاش کریں"
            onChange={(event) => setSearch(event.target.value)}
            placeholder="تلاش کریں"
            value={search}
          />
        </InputGroup>
      </div>
      <div className="flex flex-col gap-2">
        {filteredTemplates.length ? (
          <ItemGroup className="gap-2">
            {filteredTemplates.map((template) => {
              const placeholders = extractPlaceholders(template.content, placeholderIndex);
              const summary = getTemplateSummary(template.content);

              return (
                <Item
                  key={template.id}
                  render={
                    <Link to="/templates/$templateId" params={{ templateId: `${template.id}` }} />
                  }
                  dir="rtl"
                  variant="outline"
                  className="bg-muted"
                >
                  <ItemContent className="min-w-0" lang="ur">
                    <ItemTitle>{template.name}</ItemTitle>
                    <ItemDescription className="line-clamp-1 text-xs">
                      Updated {formatDate(template.updatedAt)}
                    </ItemDescription>
                    <ItemDescription className="line-clamp-2">
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
          <Empty className="min-h-80 border">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={LegalDocument01Icon} />
              </EmptyMedia>
              <EmptyTitle>No templates found</EmptyTitle>
              <EmptyDescription>
                Create a template or clear search to see saved templates.
              </EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              <Button nativeButton={false} render={<Link to="/templates/new" />} variant="outline">
                <HugeiconsIcon data-icon="inline-start" icon={Add01Icon} />
                New template
              </Button>
            </EmptyContent>
          </Empty>
        )}
      </div>
    </main>
  );
}

function TemplatesSkeleton() {
  return (
    <main className="flex flex-col gap-4 p-4 lg:p-6">
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-2">
          <Skeleton className="h-7 w-32" />
          <Skeleton className="h-4 w-64" />
        </div>
        <Skeleton className="h-8 w-28" />
      </div>
      <Skeleton className="h-[34rem]" />
    </main>
  );
}

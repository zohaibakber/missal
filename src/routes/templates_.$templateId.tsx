import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { TemplateEditorForm } from "#/components/template-editor-form";
import { Skeleton } from "#/components/ui/skeleton";

export const Route = createFileRoute("/templates_/$templateId")({
  component: RouteComponent,
});

function RouteComponent() {
  const { templateId } = Route.useParams();
  const numericTemplateId = Number(templateId);

  return (
    <ClientOnly fallback={<TemplateEditorSkeleton />}>
      <TemplateEditorForm
        templateId={Number.isInteger(numericTemplateId) ? numericTemplateId : 0}
      />
    </ClientOnly>
  );
}

function TemplateEditorSkeleton() {
  return (
    <main className="flex flex-col gap-4 p-4">
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-2">
          <Skeleton className="h-8 w-24" />
          <Skeleton className="h-7 w-40" />
          <Skeleton className="h-4 w-80" />
        </div>
        <div className="flex gap-2">
          <Skeleton className="size-8" />
          <Skeleton className="h-8 w-24" />
        </div>
      </div>
      <Skeleton className="h-[42rem]" />
    </main>
  );
}

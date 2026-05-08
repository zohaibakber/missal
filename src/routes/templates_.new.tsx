import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { TemplateEditorForm } from "#/components/template-editor-form";
import { Skeleton } from "#/components/ui/skeleton";

export const Route = createFileRoute("/templates_/new")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <ClientOnly fallback={<TemplateEditorSkeleton />}>
      <TemplateEditorForm />
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
        <Skeleton className="h-8 w-24" />
      </div>
      <Skeleton className="h-[42rem]" />
    </main>
  );
}

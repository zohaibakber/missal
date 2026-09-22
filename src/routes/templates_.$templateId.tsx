import { createFileRoute } from "@tanstack/react-router";
import { TemplateEditorForm } from "#/components/template-editor-form";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import { Button } from "#/components/ui/button";
import { Link } from "@tanstack/react-router";
import { LegalDocument01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { parseTemplateId } from "#/lib/ids";

export const Route = createFileRoute("/templates_/$templateId")({
  component: RouteComponent,
});

function RouteComponent() {
  const { templateId } = Route.useParams();
  const id = parseTemplateId(templateId);

  if (!id) {
    return (
      <div className="p-6">
        <Empty className="min-h-[28rem]" variant="outline">
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
      </div>
    );
  }

  return <TemplateEditorForm templateId={id} />;
}

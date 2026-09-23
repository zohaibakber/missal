import { createFileRoute } from "@tanstack/react-router";
import { TemplateEditorForm, TemplateNotFound } from "#/components/template-editor-form";
import { parseTemplateId } from "#/lib/ids";

export const Route = createFileRoute("/templates/$templateId")({
  component: RouteComponent,
});

function RouteComponent() {
  const { templateId } = Route.useParams();
  const id = parseTemplateId(templateId);
  return id ? <TemplateEditorForm templateId={id} /> : <TemplateNotFound />;
}

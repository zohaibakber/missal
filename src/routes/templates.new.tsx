import { createFileRoute } from "@tanstack/react-router";
import { TemplateEditorForm } from "#/components/template-editor-form";

export const Route = createFileRoute("/templates/new")({
  component: RouteComponent,
});

function RouteComponent() {
  return <TemplateEditorForm />;
}

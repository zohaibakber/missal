import { createFileRoute, Outlet } from "@tanstack/react-router";
import { RailLayout } from "#/components/rail-layout";
import { TemplateList } from "#/components/template-list";

export const Route = createFileRoute("/templates")({ component: TemplatesLayout });

function TemplatesLayout() {
  return (
    <RailLayout label="Templates" rail={<TemplateList />}>
      <Outlet />
    </RailLayout>
  );
}

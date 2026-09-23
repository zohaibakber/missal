import { createFileRoute, Outlet } from "@tanstack/react-router";
import { SplitView, SplitViewDetail, SplitViewList } from "#/components/split-view";
import { TemplateList } from "#/components/template-list";

export const Route = createFileRoute("/templates")({ component: TemplatesLayout });

function TemplatesLayout() {
  return (
    <SplitView id="templates">
      <SplitViewList>
        <TemplateList />
      </SplitViewList>
      <SplitViewDetail>
        <Outlet />
      </SplitViewDetail>
    </SplitView>
  );
}

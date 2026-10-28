import { createFileRoute, Link } from "@tanstack/react-router";
import { Add01Icon, LegalDocument01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { AppBreadcrumb } from "#/components/app-breadcrumb";
import { Pane, PaneBody, PaneHeader } from "#/components/pane";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Button } from "#/components/ui/button";
import { Empty, EmptyContent, EmptyHeader, EmptyMedia, EmptyTitle } from "#/components/ui/empty";

export const Route = createFileRoute("/templates/")({ component: TemplatePage });

function TemplatePage() {
  return (
    <Pane>
      <AppBreadcrumb crumbs={[{ label: "Templates" }]} />
      <PaneHeader />
      <PaneBody className="flex">
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} strokeWidth={2} />
            </EmptyMedia>
            <EmptyTitle>Select a template</EmptyTitle>
          </EmptyHeader>
          <EmptyContent>
            <Button nativeButton={false} render={<Link to="/templates/new" />}>
              <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
              New template
              <ShortcutKbd id="newTemplate" />
            </Button>
          </EmptyContent>
        </Empty>
      </PaneBody>
    </Pane>
  );
}

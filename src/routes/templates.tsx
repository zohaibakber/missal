import { createFileRoute, Link } from "@tanstack/react-router";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Button } from "#/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import { Add01Icon, LegalDocument01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

export const Route = createFileRoute("/templates")({ component: TemplatePage });

function TemplatePage() {
  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <HugeiconsIcon icon={LegalDocument01Icon} />
        </EmptyMedia>
        <EmptyTitle>Select a template</EmptyTitle>
        <EmptyDescription>
          Pick one from the list, or press <ShortcutKbd id="search" /> to search.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button nativeButton={false} render={<Link to="/templates/new" />}>
          <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
          New template
          <ShortcutKbd id="newTemplate" />
        </Button>
      </EmptyContent>
    </Empty>
  );
}

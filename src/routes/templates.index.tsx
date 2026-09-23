import { createFileRoute, Link } from "@tanstack/react-router";
import { Add01Icon, LegalDocument01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Hint } from "#/components/hint";
import { Pane, PaneBody, PaneHeader } from "#/components/pane";
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

export const Route = createFileRoute("/templates/")({ component: TemplatePage });

function TemplatePage() {
  return (
    <Pane>
      <PaneHeader />
      <PaneBody className="flex">
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={LegalDocument01Icon} strokeWidth={2} />
            </EmptyMedia>
            <EmptyTitle>No template selected</EmptyTitle>
            <EmptyDescription>
              Choose one from the list or press <ShortcutKbd id="search" /> to search.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Hint label="New template" shortcut="newTemplate">
              <Button
                variant="outline"
                size="sm"
                nativeButton={false}
                render={<Link to="/templates/new" />}
              >
                <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
                New template
              </Button>
            </Hint>
          </EmptyContent>
        </Empty>
      </PaneBody>
    </Pane>
  );
}

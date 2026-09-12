import { createFileRoute } from "@tanstack/react-router";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "#/components/ui/empty";
import { LegalDocument01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

export const Route = createFileRoute("/templates")({ component: TemplatePage });

function TemplatePage() {
  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <HugeiconsIcon icon={LegalDocument01Icon} />
        </EmptyMedia>
        <EmptyTitle>ٹیمپلیٹ منتخب کریں</EmptyTitle>
        <EmptyDescription>فہرست سے ٹیمپلیٹ کھولیں یا نیا ٹیمپلیٹ بنائیں۔</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

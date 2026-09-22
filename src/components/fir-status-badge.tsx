import { Badge } from "#/components/ui/badge";
import { getFirStatusColor, getFirStatusLabel, type FirStatus } from "#/lib/fir";
import { cn } from "#/lib/utils";

export function FirStatusDot({ status, className }: { status: FirStatus; className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={cn("size-1.5 shrink-0 rounded-full", getFirStatusColor(status), className)}
    />
  );
}

export function FirStatusBadge({ status }: { status: FirStatus }) {
  return (
    <Badge variant="outline" lang="ur" dir="rtl" title={status}>
      <FirStatusDot status={status} />
      {getFirStatusLabel(status)}
    </Badge>
  );
}

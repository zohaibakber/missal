import type { ReactNode } from "react";
import { Edit02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { FirStatusLabel } from "#/components/fir-status-badge";
import { Hint } from "#/components/hint";
import { RailHeader, RailTitle } from "#/components/rail-layout";
import { Button } from "#/components/ui/button";
import { formatDate } from "#/lib/date";
import { FIR_LABELS, type FirRecord } from "#/lib/fir";

function Property({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[6.5rem_minmax(0,1fr)] items-baseline gap-2 px-4 py-0.5">
      <dt lang="ur" className="truncate text-ur-sm text-muted-foreground">
        {label}
      </dt>
      <dd className="min-w-0 truncate text-sm">{children}</dd>
    </div>
  );
}

function UrduValues({ values }: { values: readonly string[] }) {
  return (
    <ul className="flex flex-col">
      {values.map((value, index) => (
        <li key={index} lang="ur" dir="rtl" className="truncate text-ur-sm">
          {value}
        </li>
      ))}
    </ul>
  );
}

function DateValue({ value }: { value: string }) {
  return value ? (
    <span className="tabular-nums">{formatDate(value)}</span>
  ) : (
    <span className="text-muted-foreground">—</span>
  );
}

export function FirDetails({ fir, onEdit }: { fir: FirRecord; onEdit: () => void }) {
  return (
    <section aria-labelledby="fir-details-title" className="pb-3">
      <RailHeader>
        <RailTitle id="fir-details-title">Details</RailTitle>
        <Hint label="Edit details" shortcut="editFir">
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            aria-label="Edit details"
            onClick={onEdit}
          >
            <HugeiconsIcon icon={Edit02Icon} strokeWidth={2} />
          </Button>
        </Hint>
      </RailHeader>
      <dl dir="rtl" className="flex flex-col">
        <Property label={FIR_LABELS.status}>
          <FirStatusLabel status={fir.status} className="max-w-full" />
        </Property>
        <Property label={FIR_LABELS.date}>
          <DateValue value={fir.date} />
        </Property>
        <Property label={FIR_LABELS.incident_date}>
          <DateValue value={fir.incident_date} />
        </Property>
        <Property label={FIR_LABELS.arrest_date}>
          <DateValue value={fir.arrest_date} />
        </Property>
        <Property label={FIR_LABELS.accused}>
          <UrduValues values={fir.accused} />
        </Property>
        {fir.witness.length ? (
          <Property label={FIR_LABELS.witness}>
            <UrduValues values={fir.witness} />
          </Property>
        ) : null}
        {fir.investigation_officer ? (
          <Property label={FIR_LABELS.investigation_officer}>
            <UrduValues values={[fir.investigation_officer]} />
          </Property>
        ) : null}
        {fir.NIC ? (
          <Property label={FIR_LABELS.NIC}>
            <span className="tabular-nums">{fir.NIC}</span>
          </Property>
        ) : null}
        {fir.mobile ? (
          <Property label={FIR_LABELS.mobile}>
            <span className="tabular-nums">{fir.mobile}</span>
          </Property>
        ) : null}
      </dl>
    </section>
  );
}

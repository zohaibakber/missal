import { FIR_STATUS_LABELS, type FirStatus } from "#/lib/fir";
import { cn } from "#/lib/utils";

const statusGlyphs = {
  Open: { className: "text-status-open", fill: null },
  "Under Investigation": {
    className: "text-status-investigating",
    fill: "M7 7V3.5a3.5 3.5 0 0 1 0 7Z",
  },
  "Challan Submitted": {
    className: "text-status-challan",
    fill: "M7 7V3.5a3.5 3.5 0 1 1-3.5 3.5Z",
  },
  Closed: { className: "text-status-closed", fill: null },
} as const satisfies Record<FirStatus, { className: string; fill: string | null }>;

export function FirStatusIcon({ status, className }: { status: FirStatus; className?: string }) {
  const glyph = statusGlyphs[status];
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 14 14"
      fill="none"
      className={cn("size-3.5 shrink-0", glyph.className, className)}
    >
      {status === "Closed" ? (
        <>
          <circle cx="7" cy="7" r="6" fill="currentColor" />
          <path
            d="m4.5 7.2 1.7 1.7 3.4-3.6"
            stroke="var(--background)"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </>
      ) : (
        <>
          <circle cx="7" cy="7" r="5.25" stroke="currentColor" strokeWidth="1.5" />
          {glyph.fill ? <path d={glyph.fill} fill="currentColor" /> : null}
        </>
      )}
    </svg>
  );
}

export function FirStatusLabel({ status, className }: { status: FirStatus; className?: string }) {
  return (
    <span
      dir="rtl"
      lang="ur"
      className={cn("inline-flex min-w-0 items-center gap-1.5 text-ur-sm", className)}
    >
      <FirStatusIcon status={status} />
      <span className="truncate">{FIR_STATUS_LABELS[status]}</span>
    </span>
  );
}

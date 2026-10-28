import { Schema } from "effect";
import { FirId } from "#/lib/ids";
import { NonEmptyTrimmedString, TrimmedString } from "#/lib/schema";

export { FirId } from "#/lib/ids";

export const FIR_STATUS_OPTIONS = [
  "Open",
  "Under Investigation",
  "Challan Submitted",
  "Closed",
] as const;

export const FirStatus = Schema.Literals(FIR_STATUS_OPTIONS);

export type FirStatus = typeof FirStatus.Type;

export class FirRecord extends Schema.Class<FirRecord>("FirRecord")({
  id: FirId,
  fir_no: NonEmptyTrimmedString,
  date: NonEmptyTrimmedString,
  offence: NonEmptyTrimmedString,
  accused: Schema.NonEmptyArray(NonEmptyTrimmedString),
  witness: Schema.Array(NonEmptyTrimmedString),
  zimni: Schema.Array(NonEmptyTrimmedString),
  NIC: TrimmedString,
  mobile: TrimmedString,
  incident_date: NonEmptyTrimmedString,
  arrest_date: TrimmedString,
  investigation_officer: TrimmedString,
  status: FirStatus,
}) {}

export class FirSummary extends Schema.Class<FirSummary>("FirSummary")({
  id: FirId,
  fir_no: NonEmptyTrimmedString,
  date: NonEmptyTrimmedString,
  offence: NonEmptyTrimmedString,
  incident_date: NonEmptyTrimmedString,
  arrest_date: TrimmedString,
  status: FirStatus,
}) {}

export class FirCreateInput extends Schema.Class<FirCreateInput>("FirCreateInput")({
  fir_no: NonEmptyTrimmedString,
  date: NonEmptyTrimmedString,
  offence: NonEmptyTrimmedString,
  accused: Schema.NonEmptyArray(NonEmptyTrimmedString),
  witness: Schema.Array(NonEmptyTrimmedString),
  zimni: Schema.Array(NonEmptyTrimmedString),
  NIC: TrimmedString,
  mobile: TrimmedString,
  incident_date: NonEmptyTrimmedString,
  arrest_date: TrimmedString,
  investigation_officer: TrimmedString,
  status: FirStatus,
}) {}

export class FirUpdateInput extends Schema.Class<FirUpdateInput>("FirUpdateInput")({
  id: FirId,
  fir_no: NonEmptyTrimmedString,
  date: NonEmptyTrimmedString,
  offence: NonEmptyTrimmedString,
  accused: Schema.NonEmptyArray(NonEmptyTrimmedString),
  witness: Schema.Array(NonEmptyTrimmedString),
  zimni: Schema.Array(NonEmptyTrimmedString),
  NIC: TrimmedString,
  mobile: TrimmedString,
  incident_date: NonEmptyTrimmedString,
  arrest_date: TrimmedString,
  investigation_officer: TrimmedString,
  status: FirStatus,
}) {}

export const FIR_STATUS_LABELS = {
  Open: "زیر التوا",
  "Under Investigation": "زیر تفتیش",
  "Challan Submitted": "چالان جمع",
  Closed: "بند",
} as const satisfies Record<FirStatus, string>;

export const FIR_LABELS = {
  fir_no: "ایف آئی آر نمبر",
  offence: "جرم",
  status: "حالت",
  date: "تاریخ ایف آئی آر",
  incident_date: "تاریخ وقوعہ",
  arrest_date: "تاریخ گرفتاری",
  accused: "ملزمان",
  witness: "گواہان",
  zimni: "ضمنی",
  NIC: "شناختی کارڈ نمبر",
  mobile: "موبائل نمبر",
  investigation_officer: "تفتیشی افسر",
} as const;

export type FirFormValues = {
  fir_no: string;
  date: string;
  offence: string;
  accused: string[];
  witness: string[];
  zimni: string[];
  NIC: string;
  mobile: string;
  incident_date: string;
  arrest_date: string;
  investigation_officer: string;
  status: FirStatus;
};

export const createEmptyFirRecord = (): FirFormValues => ({
  fir_no: "",
  date: "",
  offence: "",
  accused: [""],
  witness: [],
  zimni: [],
  NIC: "",
  mobile: "",
  incident_date: "",
  arrest_date: "",
  investigation_officer: "",
  status: "Open",
});

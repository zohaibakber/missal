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

const firFields = {
  fir_no: NonEmptyTrimmedString,
  complainant: TrimmedString,
  date: NonEmptyTrimmedString,
  offence: NonEmptyTrimmedString,
  accused: Schema.NonEmptyArray(NonEmptyTrimmedString),
  witness: Schema.Array(NonEmptyTrimmedString),
  zimni: Schema.Array(NonEmptyTrimmedString),
  incident_date: NonEmptyTrimmedString,
  arrest_date: TrimmedString,
  investigation_officer: TrimmedString,
  writer: TrimmedString,
  recovery: TrimmedString,
  body: TrimmedString,
  imei_1: TrimmedString,
  imei_2: TrimmedString,
  sim_1: TrimmedString,
  sim_2: TrimmedString,
  status: FirStatus,
};

export class FirRecord extends Schema.Class<FirRecord>("FirRecord")({
  id: FirId,
  ...firFields,
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

export class FirCreateInput extends Schema.Class<FirCreateInput>("FirCreateInput")(firFields) {}

export class FirUpdateInput extends Schema.Class<FirUpdateInput>("FirUpdateInput")({
  id: FirId,
  ...firFields,
}) {}

export const FIR_STATUS_LABELS = {
  Open: "زیر التوا",
  "Under Investigation": "زیر تفتیش",
  "Challan Submitted": "چالان جمع",
  Closed: "بند",
} as const satisfies Record<FirStatus, string>;

export const FIR_LABELS = {
  fir_no: "ایف آئی آر نمبر",
  complainant: "مدعی مقدمہ",
  offence: "جرم",
  status: "حالت",
  date: "تاریخ ایف آئی آر",
  incident_date: "تاریخ وقوعہ",
  arrest_date: "تاریخ گرفتاری",
  accused: "ملزمان",
  witness: "گواہان",
  zimni: "ضمنی",
  investigation_officer: "تفتیشی افسر",
  writer: "تحریر کنندہ",
  recovery: "برآمدگی",
  body: "متن",
  imei_1: "ایمی نمبر 1",
  imei_2: "ایمی نمبر 2",
  sim_1: "سم نمبر 1",
  sim_2: "سم نمبر 2",
} as const;

export type FirFormValues = {
  fir_no: string;
  complainant: string;
  date: string;
  offence: string;
  accused: string[];
  witness: string[];
  zimni: string[];
  incident_date: string;
  arrest_date: string;
  investigation_officer: string;
  writer: string;
  recovery: string;
  body: string;
  imei_1: string;
  imei_2: string;
  sim_1: string;
  sim_2: string;
  status: FirStatus;
};

export const createEmptyFirRecord = (): FirFormValues => ({
  fir_no: "",
  complainant: "",
  date: "",
  offence: "",
  accused: [""],
  witness: [],
  zimni: [],
  incident_date: "",
  arrest_date: "",
  investigation_officer: "",
  writer: "",
  recovery: "",
  body: "",
  imei_1: "",
  imei_2: "",
  sim_1: "",
  sim_2: "",
  status: "Open",
});

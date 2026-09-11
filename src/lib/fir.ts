import { Match, Schema } from "effect";
import { FirId, TemplateId } from "#/lib/ids";
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
  accused: NonEmptyTrimmedString,
  witness: TrimmedString,
  NIC: TrimmedString,
  mobile: TrimmedString,
  incident_date: NonEmptyTrimmedString,
  arrest_date: TrimmedString,
  investigation_officer: TrimmedString,
  status: FirStatus,
  templateId: Schema.optionalKey(TemplateId),
  content: Schema.optionalKey(Schema.String),
}) {}

export class FirCreateInput extends Schema.Class<FirCreateInput>("FirCreateInput")({
  fir_no: NonEmptyTrimmedString,
  date: NonEmptyTrimmedString,
  offence: NonEmptyTrimmedString,
  accused: NonEmptyTrimmedString,
  witness: TrimmedString,
  NIC: TrimmedString,
  mobile: TrimmedString,
  incident_date: NonEmptyTrimmedString,
  arrest_date: TrimmedString,
  investigation_officer: TrimmedString,
  status: FirStatus,
  templateId: Schema.optionalKey(TemplateId),
  content: Schema.optionalKey(Schema.String),
}) {}

export class FirUpdateInput extends Schema.Class<FirUpdateInput>("FirUpdateInput")({
  id: FirId,
  fir_no: NonEmptyTrimmedString,
  date: NonEmptyTrimmedString,
  offence: NonEmptyTrimmedString,
  accused: NonEmptyTrimmedString,
  witness: TrimmedString,
  NIC: TrimmedString,
  mobile: TrimmedString,
  incident_date: NonEmptyTrimmedString,
  arrest_date: TrimmedString,
  investigation_officer: TrimmedString,
  status: FirStatus,
  templateId: Schema.optionalKey(TemplateId),
  content: Schema.optionalKey(Schema.String),
}) {}

export class FirDocumentUpdateInput extends Schema.Class<FirDocumentUpdateInput>(
  "FirDocumentUpdateInput",
)({
  id: FirId,
  content: Schema.String,
  templateId: Schema.optionalKey(TemplateId),
}) {}

export const getFirStatusColor = (status: FirRecord["status"]) =>
  Match.value(status).pipe(
    Match.when("Open", () => "bg-amber-500"),
    Match.when("Under Investigation", () => "bg-blue-500"),
    Match.when("Challan Submitted", () => "bg-violet-500"),
    Match.when("Closed", () => "bg-emerald-500"),
    Match.exhaustive,
  );

export const getFirStatusLabel = (status: FirRecord["status"]) =>
  Match.value(status).pipe(
    Match.when("Open", () => "زیر التوا"),
    Match.when("Under Investigation", () => "زیر تفتیش"),
    Match.when("Challan Submitted", () => "چالان جمع"),
    Match.when("Closed", () => "بند"),
    Match.exhaustive,
  );

export type FirFormValues = {
  fir_no: string;
  date: string;
  offence: string;
  accused: string;
  witness: string;
  NIC: string;
  mobile: string;
  incident_date: string;
  arrest_date: string;
  investigation_officer: string;
  status: FirStatus;
  templateId?: TemplateId;
  content?: string;
};

export const createEmptyFirRecord = (): FirFormValues => ({
  fir_no: "",
  date: "",
  offence: "",
  accused: "",
  witness: "",
  NIC: "",
  mobile: "",
  incident_date: "",
  arrest_date: "",
  investigation_officer: "",
  status: "Open",
  content: "",
});

export const normalizeFirText = (value: unknown) => {
  if (typeof value === "string") {
    return value.replace(/\s+/g, " ").trim();
  }

  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {
    return `${value}`.replace(/\s+/g, " ").trim();
  }

  return "";
};

export const sanitizePhoneLikeValue = (value: unknown) =>
  normalizeFirText(value)
    .replace(/[^\d+\-\s]/g, "")
    .trim();

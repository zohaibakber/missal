import { Match, Schema } from "effect";
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

export const getFirStatusColor = (status: FirRecord["status"]) =>
  Match.value(status).pipe(
    Match.when("Open", () => "bg-status-open"),
    Match.when("Under Investigation", () => "bg-status-investigating"),
    Match.when("Challan Submitted", () => "bg-status-challan"),
    Match.when("Closed", () => "bg-status-closed"),
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

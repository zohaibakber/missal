import { z } from "zod";

export const FIR_STATUS_OPTIONS = [
  "Open",
  "Under Investigation",
  "Challan Submitted",
  "Closed",
] as const;

export const firStatusSchema = z.enum(FIR_STATUS_OPTIONS);

export const firSchema = z.object({
  id: z.number().int().positive(),
  fir_no: z.string().trim().min(1, "FIR number is required."),
  date: z.string().trim().min(1, "FIR date is required."),
  offence: z.string().trim().min(1, "Offence is required."),
  accused: z.string().trim().min(1, "Accused is required."),
  witness: z.string().trim(),
  NIC: z.string().trim(),
  mobile: z.string().trim(),
  incident_date: z.string().trim().min(1, "Incident date is required."),
  arrest_date: z.string().trim().catch(""),
  investigation_officer: z.string().trim().catch(""),
  status: firStatusSchema,
  templateId: z.number().int().positive().optional(),
  content: z.string().optional(),
});

export type FirRecord = z.infer<typeof firSchema>;

export const getFirStatusColor = (status: FirRecord["status"]) => {
  switch (status) {
    case "Open":
      return "bg-amber-500";
    case "Under Investigation":
      return "bg-blue-500";
    case "Challan Submitted":
      return "bg-violet-500";
    case "Closed":
      return "bg-emerald-500";
    default:
      return "bg-muted-foreground/64";
  }
};

export const getFirStatusLabel = (status: FirRecord["status"]) => {
  switch (status) {
    case "Open":
      return "زیر التوا";
    case "Under Investigation":
      return "زیر تفتیش";
    case "Challan Submitted":
      return "چالان جمع";
    case "Closed":
      return "بند";
    default:
      return status;
  }
};

export const createEmptyFirRecord = (): Omit<FirRecord, "id"> => ({
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
  templateId: undefined,
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

export const parseDateLikeValue = (value: unknown) => normalizeFirText(value);

import { z } from "zod";
import type { FirRecord } from "#/lib/fir";

export const templateRecordSchema = z.object({
  id: z.number().int().positive(),
  name: z.string().trim().min(1, "Template name is required."),
  content: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const firPlaceholderValueSchema = z.object({
  id: z.string().min(1),
  firId: z.number().int().positive(),
  placeholder: z.string().trim().min(1),
  value: z.string(),
  updatedAt: z.string(),
});

export type TemplateRecord = z.infer<typeof templateRecordSchema>;
export type FirPlaceholderValue = z.infer<typeof firPlaceholderValueSchema>;

export const LEGACY_PLACEHOLDER_PATTERN = /«([^»]+)»/g;
export const AT_PLACEHOLDER_PATTERN = /@([^@\r\n<>]{1,120})@/g;
export const PLACEHOLDER_PATTERN = AT_PLACEHOLDER_PATTERN;

const placeholderAliases: Record<string, keyof FirRecord> = {
  مقدمہ_نمبر: "fir_no",
  "مقدمہ نمبر": "fir_no",
  fir_no: "fir_no",
  Date_FIR: "date",
  "Date FIR": "date",
  "تاریخ ایف آئی آر": "date",
  تاریخ_ووقت_وقوعہ: "incident_date",
  "تاریخ ووقت وقوعہ": "incident_date",
  تاریخ_گرفتاری: "arrest_date",
  "تاریخ گرفتاری": "arrest_date",
  جرم_: "offence",
  جرم: "offence",
  نام_ملزم_و_سکونت_: "accused",
  "نام ملزم و سکونت": "accused",
  گواہان__1: "witness",
  "گواہان  1": "witness",
  گواہان2: "witness",
  witness: "witness",
  شناختی_کارڈ: "NIC",
  NIC: "NIC",
  موبائل: "mobile",
  mobile: "mobile",
  تفتیشی_: "investigation_officer",
  تفتیشی: "investigation_officer",
  "تفتیشی افسر": "investigation_officer",
};

export function normalizePlaceholderName(value: string) {
  return value
    .replace(/[\u200e\u200f\u202a-\u202e\u2066-\u2069]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

export function extractPlaceholders(content: string) {
  const placeholders: string[] = [];

  for (const pattern of [AT_PLACEHOLDER_PATTERN, LEGACY_PLACEHOLDER_PATTERN]) {
    for (const match of content.matchAll(pattern)) {
      const placeholder = normalizePlaceholderName(match[1] ?? "");

      if (placeholder && !placeholders.includes(placeholder)) {
        placeholders.push(placeholder);
      }
    }
  }

  return placeholders;
}

export function getCorePlaceholderValue(placeholder: string, fir: FirRecord) {
  const field = placeholderAliases[normalizePlaceholderName(placeholder)];

  if (!field) {
    return undefined;
  }

  return String(fir[field] ?? "");
}

export function getPlaceholderValueId(firId: number, placeholder: string) {
  return `${firId}:${normalizePlaceholderName(placeholder)}`;
}

export function buildTemplateValues({
  extraValues,
  fir,
  placeholders,
  sharedValues = {},
}: {
  extraValues: FirPlaceholderValue[];
  fir: FirRecord;
  placeholders: string[];
  sharedValues?: Record<string, string>;
}) {
  const values: Record<string, string> = {};

  for (const placeholder of placeholders) {
    const coreValue = getCorePlaceholderValue(placeholder, fir);

    if (coreValue !== undefined) {
      values[placeholder] = coreValue;
      continue;
    }

    const extraValue = extraValues.find(
      (value) => normalizePlaceholderName(value.placeholder) === placeholder,
    );

    if (extraValue) {
      values[placeholder] = extraValue.value;
      continue;
    }

    const sharedValue =
      sharedValues[placeholder] ?? sharedValues[normalizePlaceholderName(placeholder)];

    if (sharedValue) {
      values[placeholder] = sharedValue;
    }
  }

  return values;
}

export function renderTemplate(content: string, values: Record<string, string>) {
  return content.replace(
    /@([^@\r\n<>]{1,120})@|«([^»]+)»/g,
    (token, atPlaceholder, legacyPlaceholder) => {
      const placeholder = normalizePlaceholderName(atPlaceholder || legacyPlaceholder);
      const value = values[placeholder];

      return value?.trim() ? value : token;
    },
  );
}

export function getMissingPlaceholders(placeholders: string[], values: Record<string, string>) {
  return placeholders.filter((placeholder) => !values[placeholder]?.trim());
}

export function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function renderTemplateHtml(content: string, values: Record<string, string>) {
  return content.replace(
    /@([^@\r\n<>]{1,120})@|«([^»]+)»/g,
    (token, atPlaceholder, legacyPlaceholder) => {
      const placeholder = normalizePlaceholderName(atPlaceholder || legacyPlaceholder);
      const value = values[placeholder];

      return value?.trim()
        ? escapeHtml(value).replace(/\n/g, "<br>")
        : `<span data-placeholder="true">${escapeHtml(token)}</span>`;
    },
  );
}

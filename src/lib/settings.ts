import { z } from "zod";
import { normalizePlaceholderName } from "#/lib/templates";

export const SHARED_PLACEHOLDER_FIELDS = [
  {
    key: "policeStation",
    label: "تھانہ نام",
    placeholder: "تھانہ سٹی",
    templatePlaceholders: ["تھانہ_نام_", "تھانہ نام"],
  },
  {
    key: "district",
    label: "ضلع نام",
    placeholder: "لاہور",
    templatePlaceholders: ["ضلع_نام_", "ضلع نام"],
  },
  {
    key: "shoName",
    label: "SHO نام",
    placeholder: "نام ایس ایچ او",
    templatePlaceholders: ["SHO_نام", "SHO نام"],
  },
  {
    key: "dspName",
    label: "DSP نام",
    placeholder: "نام ڈی ایس پی",
    templatePlaceholders: ["DSP_نام", "DSP نام"],
  },
  {
    key: "investigationOfficer",
    label: "تفتیشی افسر",
    placeholder: "نام تفتیشی افسر",
    templatePlaceholders: ["تفتیشی_", "تفتیشی"],
  },
] as const;

export type SharedPlaceholderKey = (typeof SHARED_PLACEHOLDER_FIELDS)[number]["key"];

export const appSettingsSchema = z.object({
  id: z.literal("default"),
  sharedPlaceholders: z.record(z.string(), z.string()),
  updatedAt: z.string(),
});

export type AppSettings = z.infer<typeof appSettingsSchema>;

export function createDefaultAppSettings(): AppSettings {
  return {
    id: "default",
    sharedPlaceholders: {},
    updatedAt: new Date().toISOString(),
  };
}

export function normalizeSharedPlaceholderSettings(sharedPlaceholders: Record<string, string>) {
  const legacyKeys: Record<string, SharedPlaceholderKey> = {
    تھانہ_نام_: "policeStation",
    ضلع_نام_: "district",
    SHO_نام: "shoName",
    DSP_نام: "dspName",
    تفتیشی_: "investigationOfficer",
  };
  const normalized = { ...sharedPlaceholders };

  for (const [legacyKey, englishKey] of Object.entries(legacyKeys)) {
    if (normalized[englishKey]?.trim() || !normalized[legacyKey]?.trim()) {
      delete normalized[legacyKey];
      continue;
    }

    normalized[englishKey] = normalized[legacyKey];
    delete normalized[legacyKey];
  }

  return normalized;
}

export function buildSharedPlaceholderValues(sharedPlaceholders: Record<string, string>) {
  const values: Record<string, string> = {};

  for (const field of SHARED_PLACEHOLDER_FIELDS) {
    const value = sharedPlaceholders[field.key];

    if (!value?.trim()) {
      continue;
    }

    for (const templatePlaceholder of field.templatePlaceholders) {
      values[normalizePlaceholderName(templatePlaceholder)] = value;
    }
  }

  return values;
}

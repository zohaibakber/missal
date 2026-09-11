import { Schema } from "effect";
import type { FirRecord } from "#/lib/fir";
import { FirId, PlaceholderId, TemplateId } from "#/lib/ids";
import {
  CORE_PLACEHOLDER_FIELDS,
  type Placeholder,
  type PlaceholderIndex,
  indexPlaceholders,
  isCorePlaceholderKey,
  isPlaceholderToken,
  resolvePlaceholder,
  resolvePlaceholderKey,
} from "#/lib/placeholder";
import { IsoDateTimeString, NonEmptyTrimmedString } from "#/lib/schema";

export { TemplateId } from "#/lib/ids";

export class TemplateRecord extends Schema.Class<TemplateRecord>("TemplateRecord")({
  id: TemplateId,
  name: NonEmptyTrimmedString,
  content: Schema.String,
  createdAt: IsoDateTimeString,
  updatedAt: IsoDateTimeString,
}) {}

export class TemplateCreateInput extends Schema.Class<TemplateCreateInput>("TemplateCreateInput")({
  name: NonEmptyTrimmedString,
  content: Schema.String,
}) {}

export class TemplateUpdateInput extends Schema.Class<TemplateUpdateInput>("TemplateUpdateInput")({
  id: TemplateId,
  name: NonEmptyTrimmedString,
  content: Schema.String,
}) {}

export class FirPlaceholderValue extends Schema.Class<FirPlaceholderValue>("FirPlaceholderValue")({
  firId: FirId,
  placeholderId: PlaceholderId,
  value: Schema.String,
  updatedAt: IsoDateTimeString,
}) {}

export class FirPlaceholderValueUpsertInput extends Schema.Class<FirPlaceholderValueUpsertInput>(
  "FirPlaceholderValueUpsertInput",
)({
  firId: FirId,
  placeholderId: PlaceholderId,
  value: Schema.String,
}) {}

export class FirPlaceholderValueRemoveInput extends Schema.Class<FirPlaceholderValueRemoveInput>(
  "FirPlaceholderValueRemoveInput",
)({
  firId: FirId,
  placeholderId: PlaceholderId,
}) {}

export const PLACEHOLDER_PATTERN = /@([^@\r\n<>]{1,120})@/g;

function getCatalogIndex(
  catalog: readonly Placeholder[] | PlaceholderIndex = [],
): PlaceholderIndex {
  if ("byId" in catalog) {
    return catalog;
  }

  return indexPlaceholders(catalog);
}

export function extractPlaceholders(
  content: string,
  catalog: readonly Placeholder[] | PlaceholderIndex = [],
) {
  const index = getCatalogIndex(catalog);
  const placeholders: string[] = [];

  for (const match of content.matchAll(PLACEHOLDER_PATTERN)) {
    const token = (match[1] ?? "").trim();

    if (!isPlaceholderToken(token)) {
      continue;
    }

    const resolved = resolvePlaceholder(token, index);

    if (resolved && !placeholders.includes(resolved.key)) {
      placeholders.push(resolved.key);
    }
  }

  return placeholders;
}

export function getCorePlaceholderValue(placeholder: string, fir: FirRecord) {
  if (!isCorePlaceholderKey(placeholder)) {
    return undefined;
  }

  return String(fir[CORE_PLACEHOLDER_FIELDS[placeholder]] ?? "");
}

export function buildTemplateValues({
  extraValues,
  fir,
  placeholders,
  sharedValues = {},
  catalog = [],
}: {
  extraValues: readonly FirPlaceholderValue[];
  fir: FirRecord;
  placeholders: string[];
  sharedValues?: Record<string, string>;
  catalog?: readonly Placeholder[] | PlaceholderIndex;
}) {
  const index = getCatalogIndex(catalog);
  const extraByPlaceholderId = new Map(
    extraValues.map((value) => [value.placeholderId, value] as const),
  );
  const values: Record<string, string> = {};

  for (const placeholder of placeholders) {
    const resolved = resolvePlaceholder(placeholder, index);
    const key = resolved?.key ?? (resolvePlaceholderKey(placeholder, index) || placeholder);
    const coreValue = getCorePlaceholderValue(key, fir);

    if (coreValue !== undefined) {
      values[key] = coreValue;
      continue;
    }

    const extraValue = resolved ? extraByPlaceholderId.get(resolved.id) : undefined;

    if (extraValue) {
      values[key] = extraValue.value;
      continue;
    }

    const sharedValue = sharedValues[key];

    if (sharedValue) {
      values[key] = sharedValue;
    }
  }

  return values;
}

export function renderTemplate(
  content: string,
  values: Record<string, string>,
  catalog: readonly Placeholder[] | PlaceholderIndex = [],
) {
  const index = getCatalogIndex(catalog);

  return content.replace(/@([^@\r\n<>]{1,120})@/g, (token, rawPlaceholder: string) => {
    const placeholder = resolvePlaceholderKey(rawPlaceholder, index);
    const value = values[placeholder];

    return value?.trim() ? value : token;
  });
}

export function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function renderTemplateHtml(
  content: string,
  values: Record<string, string>,
  catalog: readonly Placeholder[] | PlaceholderIndex = [],
) {
  const index = getCatalogIndex(catalog);

  return content.replace(/@([^@\r\n<>]{1,120})@/g, (token, rawPlaceholder: string) => {
    const placeholder = resolvePlaceholderKey(rawPlaceholder, index);
    const value = values[placeholder];

    return value?.trim()
      ? escapeHtml(value).replace(/\n/g, "<br>")
      : `<span data-placeholder="true">${escapeHtml(token)}</span>`;
  });
}

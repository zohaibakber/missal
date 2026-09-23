import { format, isValid, parse, parseISO } from "date-fns";

const DATE_FORMAT = "dd-MM-yyyy";

const PARSE_FORMATS = [DATE_FORMAT, "dd.MM.yyyy", "MMMM d, yyyy"] as const;

export function parseDate(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return undefined;
  }

  for (const dateFormat of PARSE_FORMATS) {
    const parsed = parse(trimmed, dateFormat, new Date());

    if (isValid(parsed)) {
      return parsed;
    }
  }

  const iso = parseISO(trimmed);
  return isValid(iso) ? iso : undefined;
}

export function formatDate(value: Date | string) {
  if (value instanceof Date) {
    return isValid(value) ? format(value, DATE_FORMAT) : "";
  }

  const date = parseDate(value);
  return date ? format(date, DATE_FORMAT) : value.trim();
}

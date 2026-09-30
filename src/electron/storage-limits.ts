/** First-profile readiness budget is 8s at p95; waiters stop here instead of hanging. */
export const STORAGE_READINESS_MS = 30_000;

/** Past the worker platform's 5s terminate, so dispose can kill the thread before quit returns. */
export const STORAGE_SHUTDOWN_MS = 6_000;

/** One request running on the single storage worker plus 16 waiting. */
export const STORAGE_MAX_PENDING = 17;

/** Supported document bodies are 4 MiB; the JSON request around one is allowed twice that. */
export const STORAGE_MAX_REQUEST_BYTES = 8 * 1024 * 1024;

export const STORAGE_MAX_RETAINED_BYTES = 16 * 1024 * 1024;

const READ_DEADLINE_MS = 10_000;
const WRITE_DEADLINE_MS = 15_000;
const DOCUMENT_DEADLINE_MS = 30_000;

const WRITE_OPERATIONS = new Set([
  "Placeholder.saveGlobals",
  "Placeholder.create",
  "Placeholder.update",
  "Placeholder.remove",
  "Template.create",
  "Template.save",
  "Template.remove",
  "Fir.create",
  "Fir.update",
  "Fir.remove",
  "FirDocument.addTemplates",
  "FirDocument.save",
  "FirDocument.reorder",
  "FirDocument.remove",
  "FirPlaceholderValue.upsert",
  "FirPlaceholderValue.remove",
  "Settings.save",
  "Settings.saveFieldMarkers",
]);

const DOCUMENT_OPERATIONS = new Set([
  "Template.get",
  "Template.create",
  "Template.save",
  "FirDocument.get",
  "FirDocument.getMany",
  "FirDocument.save",
  "FirDocument.addTemplates",
]);

const TAG_PREFIX = '{"_tag":"';

/** Reads the request tag without decoding the payload; unknown requests are treated as writes. */
export function storageOperationPolicy(payload: string) {
  const end = payload.startsWith(TAG_PREFIX) ? payload.indexOf('"', TAG_PREFIX.length) : -1;
  const tag = end === -1 ? undefined : payload.slice(TAG_PREFIX.length, end);
  const write = tag === undefined || WRITE_OPERATIONS.has(tag);
  const deadlineMs =
    tag !== undefined && DOCUMENT_OPERATIONS.has(tag)
      ? DOCUMENT_DEADLINE_MS
      : write
        ? WRITE_DEADLINE_MS
        : READ_DEADLINE_MS;
  return { deadlineMs, operation: tag ?? "storage.request", write };
}

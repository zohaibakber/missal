import { createRemoteJWKSet, jwtVerify, type JWTPayload } from "jose";
import type { ApiEnv } from "../alchemy.run.ts";

const MAX_CONTENT = 16 * 1024 * 1024;
const MAX_BODY = Math.ceil((MAX_CONTENT * 4) / 3) + 64 * 1024;
const MAX_NAME = 200;
const MAX_PREVIEW = 400;
const PUBLISH_PERMISSION = "templates:publish";
const PUBLISH_ROLE = "admin";

interface Claims extends JWTPayload {
  sub: string;
  role?: string;
  roles?: string[];
  permissions?: string[];
}

interface Viewer {
  id: string;
  publisher: boolean;
}

interface Row {
  id: string;
  name: string;
  version: number;
  field_count: number;
  preview: string;
  updated_at: number;
}

interface UploadBody {
  name: string;
  fieldCount: number;
  preview: string;
  content: string;
}

const jwks = new Map<string, ReturnType<typeof createRemoteJWKSet>>();

function keySet(clientId: string) {
  let set = jwks.get(clientId);
  if (!set) {
    set = createRemoteJWKSet(new URL(`https://api.workos.com/sso/jwks/${clientId}`));
    jwks.set(clientId, set);
  }
  return set;
}

function problem(status: number, error: string): Response {
  return Response.json({ error }, { status, headers: { "cache-control": "no-store" } });
}

function publishers(env: ApiEnv): Set<string> {
  return new Set(
    env.PUBLISHERS.split(",")
      .map((id) => id.trim())
      .filter((id) => id.length > 0),
  );
}

async function viewer(request: Request, env: ApiEnv): Promise<Viewer | null> {
  const header = request.headers.get("authorization") ?? "";
  const match = /^Bearer\s+(\S+)$/i.exec(header);
  if (!match) return null;
  try {
    const { payload } = await jwtVerify<Claims>(match[1], keySet(env.WORKOS_CLIENT_ID), {
      algorithms: ["RS256"],
    });
    if (typeof payload.sub !== "string" || payload.sub.length === 0) return null;
    const roles = [payload.role, ...(payload.roles ?? [])];
    const publisher =
      publishers(env).has(payload.sub) ||
      roles.includes(PUBLISH_ROLE) ||
      (payload.permissions ?? []).includes(PUBLISH_PERMISSION);
    return { id: payload.sub, publisher };
  } catch {
    return null;
  }
}

function entry(row: Row) {
  return {
    id: row.id,
    name: row.name,
    version: row.version,
    fieldCount: row.field_count,
    preview: row.preview,
    updatedAt: row.updated_at,
  };
}

function objectKey(id: string, version: number): string {
  return `templates/${id}/${version}.msb`;
}

function decodeBase64(text: string): Uint8Array | null {
  try {
    const binary = atob(text);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
    return bytes;
  } catch {
    return null;
  }
}

async function readUpload(request: Request): Promise<UploadBody | Response> {
  const length = Number(request.headers.get("content-length") ?? "0");
  if (length > MAX_BODY) return problem(413, "template too large");
  let body: unknown;
  try {
    const text = await request.text();
    if (text.length > MAX_BODY) return problem(413, "template too large");
    body = JSON.parse(text);
  } catch {
    return problem(400, "body must be JSON");
  }
  const upload = body as Partial<UploadBody>;
  const name = typeof upload.name === "string" ? upload.name.trim() : "";
  if (name.length === 0 || name.length > MAX_NAME) return problem(400, "name is required");
  if (!Number.isInteger(upload.fieldCount) || (upload.fieldCount as number) < 0) {
    return problem(400, "fieldCount must be a whole number");
  }
  if (typeof upload.content !== "string" || upload.content.length === 0) {
    return problem(400, "content is required");
  }
  return {
    name,
    fieldCount: upload.fieldCount as number,
    preview: typeof upload.preview === "string" ? upload.preview.slice(0, MAX_PREVIEW) : "",
    content: upload.content,
  };
}

async function list(env: ApiEnv, who: Viewer): Promise<Response> {
  const { results } = await env.CATALOG.prepare(
    "SELECT id, name, version, field_count, preview, updated_at FROM templates ORDER BY name",
  ).all<Row>();
  return Response.json(
    { publisher: who.publisher, templates: results.map(entry) },
    { headers: { "cache-control": "no-store" } },
  );
}

async function download(env: ApiEnv, id: string): Promise<Response> {
  const row = await env.CATALOG.prepare("SELECT version FROM templates WHERE id = ?1")
    .bind(id)
    .first<{ version: number }>();
  if (!row) return problem(404, "not found");
  const object = await env.TEMPLATES.get(objectKey(id, row.version));
  if (!object) return problem(404, "not found");
  return new Response(object.body, {
    headers: {
      "content-type": "application/octet-stream",
      "content-length": String(object.size),
      "x-missal-version": String(row.version),
      "cache-control": "no-store",
    },
  });
}

async function publish(
  request: Request,
  env: ApiEnv,
  who: Viewer,
  id: string | null,
): Promise<Response> {
  const upload = await readUpload(request);
  if (upload instanceof Response) return upload;
  const content = decodeBase64(upload.content);
  if (!content || content.length === 0) return problem(400, "content is not base64");
  if (content.length > MAX_CONTENT) return problem(413, "template too large");
  const now = Date.now();
  if (id === null) {
    const created = crypto.randomUUID();
    await env.TEMPLATES.put(objectKey(created, 1), content);
    await env.CATALOG.prepare(
      "INSERT INTO templates (id, name, version, field_count, preview, size, published_by, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?7)",
    )
      .bind(created, upload.name, upload.fieldCount, upload.preview, content.length, who.id, now)
      .run();
    return Response.json({ id: created, version: 1 }, { status: 201 });
  }
  const current = await env.CATALOG.prepare("SELECT version FROM templates WHERE id = ?1")
    .bind(id)
    .first<{ version: number }>();
  if (!current) return problem(404, "not found");
  const version = current.version + 1;
  await env.TEMPLATES.put(objectKey(id, version), content);
  const updated = await env.CATALOG.prepare(
    "UPDATE templates SET name = ?2, version = ?3, field_count = ?4, preview = ?5, size = ?6, published_by = ?7, updated_at = ?8 WHERE id = ?1 AND version = ?9",
  )
    .bind(id, upload.name, version, upload.fieldCount, upload.preview, content.length, who.id, now, current.version)
    .run();
  if (updated.meta.changes === 0) {
    await env.TEMPLATES.delete(objectKey(id, version));
    return problem(409, "someone else published this template at the same time; try again");
  }
  await env.TEMPLATES.delete(objectKey(id, current.version));
  return Response.json({ id, version });
}

async function unpublish(env: ApiEnv, id: string): Promise<Response> {
  const removed = await env.CATALOG.prepare("DELETE FROM templates WHERE id = ?1 RETURNING version")
    .bind(id)
    .first<{ version: number }>();
  if (!removed) return problem(404, "not found");
  const listing = await env.TEMPLATES.list({ prefix: `templates/${id}/` });
  if (listing.objects.length > 0) {
    await env.TEMPLATES.delete(listing.objects.map((object) => object.key));
  }
  return new Response(null, { status: 204 });
}

const TEMPLATE = /^\/v1\/templates\/([A-Za-z0-9_-]{1,64})(\/content)?$/;

export default {
  async fetch(request: Request, env: ApiEnv): Promise<Response> {
    const { pathname } = new URL(request.url);
    if (pathname === "/health") return new Response("ok");
    if (!pathname.startsWith("/v1/")) return problem(404, "not found");
    const who = await viewer(request, env);
    if (!who) return problem(401, "sign in to use the template library");
    const method = request.method;
    if (pathname === "/v1/templates") {
      if (method === "GET") return list(env, who);
      if (method === "POST") {
        return who.publisher ? publish(request, env, who, null) : problem(403, "not a publisher");
      }
      return problem(405, "method not allowed");
    }
    const match = TEMPLATE.exec(pathname);
    if (!match) return problem(404, "not found");
    const [, id, content] = match;
    if (content) {
      return method === "GET" ? download(env, id) : problem(405, "method not allowed");
    }
    if (method === "PUT" || method === "DELETE") {
      if (!who.publisher) return problem(403, "not a publisher");
      return method === "PUT" ? publish(request, env, who, id) : unpublish(env, id);
    }
    return problem(405, "method not allowed");
  },
};

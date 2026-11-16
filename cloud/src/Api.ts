import * as Context from "effect/Context";
import * as HttpApi from "effect/http-api/HttpApi";
import * as HttpApiEndpoint from "effect/http-api/HttpApiEndpoint";
import * as HttpApiGroup from "effect/http-api/HttpApiGroup";
import * as HttpApiMiddleware from "effect/http-api/HttpApiMiddleware";
import * as HttpApiSchema from "effect/http-api/HttpApiSchema";
import * as HttpApiSecurity from "effect/http-api/HttpApiSecurity";
import * as Schema from "effect/Schema";

export const MAX_TEMPLATE_BYTES = 16 * 1024 * 1024;

export class Viewer extends Context.Service<
  Viewer,
  { readonly id: string; readonly publisher: boolean }
>()("missal/Viewer") {}

export class Unauthorized extends Schema.TaggedError<Unauthorized>()(
  "Unauthorized",
  { error: Schema.String },
  { httpApiStatus: 401 },
) {}

export class NotPublisher extends Schema.TaggedError<NotPublisher>()(
  "NotPublisher",
  { error: Schema.String },
  { httpApiStatus: 403 },
) {}

export class TemplateNotFound extends Schema.TaggedError<TemplateNotFound>()(
  "TemplateNotFound",
  { error: Schema.String },
  { httpApiStatus: 404 },
) {}

export class PublishConflict extends Schema.TaggedError<PublishConflict>()(
  "PublishConflict",
  { error: Schema.String },
  { httpApiStatus: 409 },
) {}

export class StorageFailure extends Schema.TaggedError<StorageFailure>()(
  "StorageFailure",
  { error: Schema.String },
  { httpApiStatus: 500 },
) {}

export class Authentication extends HttpApiMiddleware.Service<
  Authentication,
  { provides: Viewer }
>()("missal/Authentication", {
  security: { bearer: HttpApiSecurity.bearer },
  error: Unauthorized,
}) {}

export class PublishersOnly extends HttpApiMiddleware.Service<
  PublishersOnly,
  { requires: Viewer }
>()("missal/PublishersOnly", {
  error: NotPublisher,
}) {}

export const TemplateId = Schema.String.check(Schema.isPattern(/^[A-Za-z0-9_-]{1,64}$/));

export const Entry = Schema.Struct({
  id: Schema.String,
  name: Schema.String,
  version: Schema.Int,
  fieldCount: Schema.Int,
  preview: Schema.String,
  updatedAt: Schema.Number,
});
export type Entry = typeof Entry.Type;

export const Catalog = Schema.Struct({
  publisher: Schema.Boolean,
  templates: Schema.Array(Entry),
});

export const Upload = Schema.Struct({
  name: Schema.String.check(Schema.isTrimmed(), Schema.isNonEmpty(), Schema.isMaxLength(200)),
  fieldCount: Schema.Int.check(Schema.isGreaterThanOrEqualTo(0)),
  preview: Schema.String.check(Schema.isMaxLength(1000)),
  content: Schema.Uint8ArrayFromBase64.check(
    Schema.isNonEmpty(),
    Schema.isMaxLength(MAX_TEMPLATE_BYTES),
  ),
});
export type Upload = typeof Upload.Type;

export const Published = Schema.Struct({
  id: Schema.String,
  version: Schema.Int,
});

const TemplateContent = HttpApiSchema.WithHeaders(
  Schema.Uint8Array.pipe(HttpApiSchema.asUint8Array({ contentType: "application/octet-stream" })),
  { "x-missal-version": Schema.NumberFromString },
);

export class CatalogGroup extends HttpApiGroup.make("catalog")
  .add(
    HttpApiEndpoint.get("list", "/v1/templates", {
      success: Catalog,
      error: StorageFailure,
    }),
  )
  .add(
    HttpApiEndpoint.get("download", "/v1/templates/:id/content", {
      params: { id: TemplateId },
      success: TemplateContent,
      error: [TemplateNotFound, StorageFailure],
    }),
  )
  .middleware(Authentication) {}

export class PublishingGroup extends HttpApiGroup.make("publishing")
  .add(
    HttpApiEndpoint.post("create", "/v1/templates", {
      payload: Upload,
      success: Published.pipe(HttpApiSchema.status(201)),
      error: StorageFailure,
    }),
  )
  .add(
    HttpApiEndpoint.put("update", "/v1/templates/:id", {
      params: { id: TemplateId },
      payload: Upload,
      success: Published,
      error: [TemplateNotFound, PublishConflict, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.delete("remove", "/v1/templates/:id", {
      params: { id: TemplateId },
      success: HttpApiSchema.NoContent,
      error: [TemplateNotFound, StorageFailure],
    }),
  )
  .middleware(PublishersOnly)
  .middleware(Authentication) {}

export class HealthGroup extends HttpApiGroup.make("health").add(
  HttpApiEndpoint.get("health", "/health", {
    success: Schema.String.pipe(HttpApiSchema.asText()),
  }),
) {}

export class LibraryApi extends HttpApi.make("missal-library")
  .add(CatalogGroup)
  .add(PublishingGroup)
  .add(HealthGroup) {}

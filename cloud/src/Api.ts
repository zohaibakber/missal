import * as Context from "effect/Context";
import * as HttpApi from "effect/http-api/HttpApi";
import * as HttpApiEndpoint from "effect/http-api/HttpApiEndpoint";
import * as HttpApiGroup from "effect/http-api/HttpApiGroup";
import * as HttpApiMiddleware from "effect/http-api/HttpApiMiddleware";
import * as HttpApiSchema from "effect/http-api/HttpApiSchema";
import * as HttpApiSecurity from "effect/http-api/HttpApiSecurity";
import * as Schema from "effect/Schema";

export const MAX_TEMPLATE_BYTES = 16 * 1024 * 1024;

export const Role = Schema.Literals(["admin", "publisher", "member"]);
export type Role = typeof Role.Type;

export const canPublish = (role: Role) => role === "admin" || role === "publisher";

export class Viewer extends Context.Service<
  Viewer,
  {
    readonly id: string;
    readonly sessionId: string;
    readonly role: Role;
    readonly mustChangePassword: boolean;
  }
>()("missal/Viewer") {}

export class Unauthorized extends Schema.TaggedError<Unauthorized>()(
  "Unauthorized",
  { error: Schema.String },
  { httpApiStatus: 401 },
) {}
export class InvalidCredentials extends Schema.TaggedError<InvalidCredentials>()(
  "InvalidCredentials",
  { error: Schema.String },
  { httpApiStatus: 401 },
) {}
export class SessionExpired extends Schema.TaggedError<SessionExpired>()(
  "SessionExpired",
  { error: Schema.String },
  { httpApiStatus: 401 },
) {}
export class PasswordChangeRequired extends Schema.TaggedError<PasswordChangeRequired>()(
  "PasswordChangeRequired",
  { error: Schema.String },
  { httpApiStatus: 403 },
) {}
export class NotPublisher extends Schema.TaggedError<NotPublisher>()(
  "NotPublisher",
  { error: Schema.String },
  { httpApiStatus: 403 },
) {}
export class NotAdmin extends Schema.TaggedError<NotAdmin>()(
  "NotAdmin",
  { error: Schema.String },
  { httpApiStatus: 403 },
) {}
export class WrongPassword extends Schema.TaggedError<WrongPassword>()(
  "WrongPassword",
  { error: Schema.String },
  { httpApiStatus: 403 },
) {}
export class TemplateNotFound extends Schema.TaggedError<TemplateNotFound>()(
  "TemplateNotFound",
  { error: Schema.String },
  { httpApiStatus: 404 },
) {}
export class UserNotFound extends Schema.TaggedError<UserNotFound>()(
  "UserNotFound",
  { error: Schema.String },
  { httpApiStatus: 404 },
) {}
export class PublishConflict extends Schema.TaggedError<PublishConflict>()(
  "PublishConflict",
  { error: Schema.String },
  { httpApiStatus: 409 },
) {}
export class EmailTaken extends Schema.TaggedError<EmailTaken>()(
  "EmailTaken",
  { error: Schema.String },
  { httpApiStatus: 409 },
) {}
export class OwnAccess extends Schema.TaggedError<OwnAccess>()(
  "OwnAccess",
  { error: Schema.String },
  { httpApiStatus: 409 },
) {}
export class StorageFailure extends Schema.TaggedError<StorageFailure>()(
  "StorageFailure",
  { error: Schema.String },
  { httpApiStatus: 500 },
) {}

export class AccountLocked extends Schema.TaggedError<AccountLocked>()(
  "AccountLocked",
  { error: Schema.String, retryAfter: Schema.Int },
  { httpApiStatus: 429 },
) {}

export class Authentication extends HttpApiMiddleware.Service<
  Authentication,
  { provides: Viewer }
>()("missal/Authentication", {
  security: { bearer: HttpApiSecurity.bearer },
  error: Unauthorized,
}) {}

export class PasswordCurrent extends HttpApiMiddleware.Service<
  PasswordCurrent,
  { requires: Viewer }
>()("missal/PasswordCurrent", {
  error: PasswordChangeRequired,
}) {}

export class PublishersOnly extends HttpApiMiddleware.Service<
  PublishersOnly,
  { requires: Viewer }
>()("missal/PublishersOnly", {
  error: NotPublisher,
}) {}

export class AdminsOnly extends HttpApiMiddleware.Service<
  AdminsOnly,
  { requires: Viewer }
>()("missal/AdminsOnly", {
  error: NotAdmin,
}) {}

export const Email = Schema.String.check(
  Schema.isTrimmed(),
  Schema.isMaxLength(254),
  Schema.isPattern(/^[^\s@]+@[^\s@]+\.[^\s@]+$/),
);

export const NewPassword = Schema.RedactedFromValue(
  Schema.String.check(Schema.isMinLength(10), Schema.isMaxLength(256)),
);

export const GivenPassword = Schema.RedactedFromValue(
  Schema.String.check(Schema.isNonEmpty(), Schema.isMaxLength(256)),
);

export const Name = Schema.String.check(Schema.isTrimmed(), Schema.isMaxLength(120));

export const Account = Schema.Struct({
  id: Schema.String,
  email: Schema.String,
  name: Schema.String,
  role: Role,
  mustChangePassword: Schema.Boolean,
  disabled: Schema.Boolean,
  createdAt: Schema.Number,
});
export type Account = typeof Account.Type;

export const Session = Schema.Struct({
  accessToken: Schema.String,
  refreshToken: Schema.String,
  expiresIn: Schema.Int,
  account: Account,
});
export type Session = typeof Session.Type;

const RefreshToken = Schema.Struct({ refreshToken: Schema.String.check(Schema.isMaxLength(512)) });

export const TemplateId = Schema.String.check(Schema.isPattern(/^[A-Za-z0-9_-]{1,64}$/));
export const UserId = TemplateId;

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

export const NewUser = Schema.Struct({
  email: Email,
  name: Name,
  role: Role,
  password: NewPassword,
});
export type NewUser = typeof NewUser.Type;

export const UserChanges = Schema.Struct({
  name: Schema.optional(Name),
  role: Schema.optional(Role),
  disabled: Schema.optional(Schema.Boolean),
});
export type UserChanges = typeof UserChanges.Type;

const TemplateContent = HttpApiSchema.WithHeaders(
  Schema.Uint8Array.pipe(HttpApiSchema.asUint8Array({ contentType: "application/octet-stream" })),
  { "x-missal-version": Schema.NumberFromString },
);

export class AuthGroup extends HttpApiGroup.make("auth")
  .add(
    HttpApiEndpoint.post("signIn", "/v1/auth/sign-in", {
      payload: Schema.Struct({ email: Email, password: GivenPassword }),
      success: Session,
      error: [InvalidCredentials, AccountLocked, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("refresh", "/v1/auth/refresh", {
      payload: RefreshToken,
      success: Session,
      error: [SessionExpired, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("signOut", "/v1/auth/sign-out", {
      payload: RefreshToken,
      success: HttpApiSchema.NoContent,
      error: StorageFailure,
    }),
  ) {}

export class AccountGroup extends HttpApiGroup.make("account")
  .add(
    HttpApiEndpoint.get("me", "/v1/account", {
      success: Account,
      error: [UserNotFound, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("changePassword", "/v1/account/password", {
      payload: Schema.Struct({ currentPassword: GivenPassword, newPassword: NewPassword }),
      success: Session,
      error: [WrongPassword, UserNotFound, StorageFailure],
    }),
  )
  .middleware(Authentication) {}

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
  .middleware(PasswordCurrent)
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
  .middleware(PasswordCurrent)
  .middleware(Authentication) {}

export class UsersGroup extends HttpApiGroup.make("users")
  .add(
    HttpApiEndpoint.get("list", "/v1/users", {
      success: Schema.Array(Account),
      error: StorageFailure,
    }),
  )
  .add(
    HttpApiEndpoint.post("create", "/v1/users", {
      payload: NewUser,
      success: Account.pipe(HttpApiSchema.status(201)),
      error: [EmailTaken, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.patch("update", "/v1/users/:id", {
      params: { id: UserId },
      payload: UserChanges,
      success: Account,
      error: [UserNotFound, OwnAccess, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("resetPassword", "/v1/users/:id/password", {
      params: { id: UserId },
      payload: Schema.Struct({ password: NewPassword }),
      success: Account,
      error: [UserNotFound, StorageFailure],
    }),
  )
  .middleware(AdminsOnly)
  .middleware(PasswordCurrent)
  .middleware(Authentication) {}

export class HealthGroup extends HttpApiGroup.make("health").add(
  HttpApiEndpoint.get("health", "/health", {
    success: Schema.String.pipe(HttpApiSchema.asText()),
  }),
) {}

export class MissalApi extends HttpApi.make("missal")
  .add(AuthGroup)
  .add(AccountGroup)
  .add(CatalogGroup)
  .add(PublishingGroup)
  .add(UsersGroup)
  .add(HealthGroup) {}

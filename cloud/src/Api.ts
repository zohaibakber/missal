import * as Context from "effect/Context";
import type * as DateTime from "effect/DateTime";
import * as HttpApi from "effect/http-api/HttpApi";
import * as HttpApiEndpoint from "effect/http-api/HttpApiEndpoint";
import * as HttpApiGroup from "effect/http-api/HttpApiGroup";
import * as HttpApiMiddleware from "effect/http-api/HttpApiMiddleware";
import * as HttpApiSchema from "effect/http-api/HttpApiSchema";
import * as HttpApiSecurity from "effect/http-api/HttpApiSecurity";
import * as Schema from "effect/Schema";
import {
  CodePurpose,
  Email,
  Licence,
  Name,
  Role,
  RouteId,
  type SessionId,
  Template,
  UserId,
} from "./Models.ts";

export const MAX_TEMPLATE_BYTES = 16 * 1024 * 1024;

export class Viewer extends Context.Service<
  Viewer,
  {
    readonly id: UserId;
    readonly sessionId: SessionId;
    readonly role: Role;
    readonly licensedUntil: DateTime.Utc;
  }
>()("Viewer") {}

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
export class InvalidCode extends Schema.TaggedError<InvalidCode>()(
  "InvalidCode",
  { error: Schema.String },
  { httpApiStatus: 400 },
) {}
export class LicenceRequired extends Schema.TaggedError<LicenceRequired>()(
  "LicenceRequired",
  { error: Schema.String },
  { httpApiStatus: 402 },
) {}
export class EmailNotVerified extends Schema.TaggedError<EmailNotVerified>()(
  "EmailNotVerified",
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
>()("Authentication", {
  security: { bearer: HttpApiSecurity.bearer },
  error: Unauthorized,
}) {}

export class Licensed extends HttpApiMiddleware.Service<Licensed, { requires: Viewer }>()(
  "Licensed",
  {
    error: LicenceRequired,
  },
) {}

export class PublishersOnly extends HttpApiMiddleware.Service<
  PublishersOnly,
  { requires: Viewer }
>()("PublishersOnly", {
  error: NotPublisher,
}) {}

export class AdminsOnly extends HttpApiMiddleware.Service<AdminsOnly, { requires: Viewer }>()(
  "AdminsOnly",
  {
    error: NotAdmin,
  },
) {}

export const NewPassword = Schema.RedactedFromValue(
  Schema.String.check(Schema.isMinLength(10), Schema.isMaxLength(256)),
);

export const GivenPassword = Schema.RedactedFromValue(
  Schema.String.check(Schema.isNonEmpty(), Schema.isMaxLength(256)),
);

export const Account = Schema.Struct({
  id: UserId,
  email: Email,
  name: Name,
  role: Role,
  disabled: Schema.Boolean,
  emailVerified: Schema.Boolean,
  createdAt: Schema.DateTimeUtcFromMillis,
  licence: Licence,
});
export type Account = typeof Account.Type;

export const ManagedAccount = Schema.Struct({
  ...Account.fields,
  sessions: Schema.Int,
  lastSeenAt: Schema.NullOr(Schema.DateTimeUtcFromMillis),
});
export type ManagedAccount = typeof ManagedAccount.Type;

export const Session = Schema.Struct({
  accessToken: Schema.String,
  refreshToken: Schema.String,
  expiresIn: Schema.Int,
  account: Account,
});
export type Session = typeof Session.Type;

const RefreshToken = Schema.Struct({ refreshToken: Schema.String.check(Schema.isMaxLength(512)) });

export const Entry = Template.json;
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

export const ManagedTemplate = Schema.Struct({
  id: Schema.String,
  name: Schema.String,
  version: Schema.Int,
  fieldCount: Schema.Int,
  preview: Schema.String,
  size: Schema.Int,
  publisherName: Schema.NullOr(Schema.String),
  publisherEmail: Schema.NullOr(Schema.String),
  versionCount: Schema.Int,
  createdAt: Schema.DateTimeUtcFromMillis,
  updatedAt: Schema.DateTimeUtcFromMillis,
});
export type ManagedTemplate = typeof ManagedTemplate.Type;

export const TemplateVersion = Schema.Struct({
  version: Schema.Int,
  name: Schema.String,
  fieldCount: Schema.Int,
  size: Schema.Int,
  publisherName: Schema.NullOr(Schema.String),
  publisherEmail: Schema.NullOr(Schema.String),
  publishedAt: Schema.DateTimeUtcFromMillis,
  restoredFrom: Schema.NullOr(Schema.Int),
});
export type TemplateVersion = typeof TemplateVersion.Type;

export const SignUp = Schema.Struct({ email: Email, name: Name, password: NewPassword });
export type SignUp = typeof SignUp.Type;

const Code = Schema.Trim.check(Schema.isMaxLength(16));

export const UserChanges = Schema.Struct({
  name: Schema.optional(Name),
  role: Schema.optional(Role),
  disabled: Schema.optional(Schema.Boolean),
});
export type UserChanges = typeof UserChanges.Type;

export const NewUser = Schema.Struct({
  email: Email,
  name: Name,
  role: Role,
  password: NewPassword,
  months: Schema.Int.check(Schema.isBetween({ minimum: 0, maximum: 60 })),
});
export type NewUser = typeof NewUser.Type;

const Months = Schema.Int.check(Schema.isBetween({ minimum: 1, maximum: 60 }));
const Days = Schema.Int.check(Schema.isBetween({ minimum: 1, maximum: 90 }));

const TemplateContent = HttpApiSchema.WithHeaders(
  Schema.Uint8Array.pipe(HttpApiSchema.asUint8Array({ contentType: "application/octet-stream" })),
  { "x-missal-version": Schema.NumberFromString },
);

export class AuthGroup extends HttpApiGroup.make("auth")
  .add(
    HttpApiEndpoint.post("signUp", "/v1/auth/sign-up", {
      payload: SignUp,
      success: HttpApiSchema.Accepted,
      error: [AccountLocked, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("requestCode", "/v1/auth/codes", {
      payload: Schema.Struct({ email: Email, purpose: CodePurpose }),
      success: HttpApiSchema.Accepted,
      error: [AccountLocked, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("verifyEmail", "/v1/auth/verify-email", {
      payload: Schema.Struct({ email: Email, code: Code }),
      success: Session,
      error: [InvalidCode, AccountLocked, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("resetPassword", "/v1/auth/reset-password", {
      payload: Schema.Struct({ email: Email, code: Code, password: NewPassword }),
      success: Session,
      error: [InvalidCode, AccountLocked, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("signIn", "/v1/auth/sign-in", {
      payload: Schema.Struct({ email: Email, password: GivenPassword }),
      success: Session,
      error: [InvalidCredentials, EmailNotVerified, AccountLocked, StorageFailure],
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
      params: { id: RouteId },
      success: TemplateContent,
      error: [TemplateNotFound, StorageFailure],
    }),
  )
  .middleware(Licensed)
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
      params: { id: RouteId },
      payload: Upload,
      success: Published,
      error: [TemplateNotFound, PublishConflict, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.delete("remove", "/v1/templates/:id", {
      params: { id: RouteId },
      success: HttpApiSchema.NoContent,
      error: [TemplateNotFound, StorageFailure],
    }),
  )
  .middleware(PublishersOnly)
  .middleware(Licensed)
  .middleware(Authentication) {}

export class TemplateAdminGroup extends HttpApiGroup.make("templateAdmin")
  .add(
    HttpApiEndpoint.get("list", "/v1/admin/templates", {
      success: Schema.Array(ManagedTemplate),
      error: StorageFailure,
    }),
  )
  .add(
    HttpApiEndpoint.get("versions", "/v1/admin/templates/:id/versions", {
      params: { id: RouteId },
      success: Schema.Array(TemplateVersion),
      error: [TemplateNotFound, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("restore", "/v1/admin/templates/:id/restore", {
      params: { id: RouteId },
      payload: Schema.Struct({ version: Schema.Int.check(Schema.isGreaterThan(0)) }),
      success: Published,
      error: [TemplateNotFound, PublishConflict, StorageFailure],
    }),
  )
  .middleware(AdminsOnly)
  .middleware(Authentication) {}

export class UsersGroup extends HttpApiGroup.make("users")
  .add(
    HttpApiEndpoint.get("list", "/v1/users", {
      success: Schema.Array(ManagedAccount),
      error: StorageFailure,
    }),
  )
  .add(
    HttpApiEndpoint.post("create", "/v1/users", {
      payload: NewUser,
      success: ManagedAccount.pipe(HttpApiSchema.status(201)),
      error: [EmailTaken, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.patch("update", "/v1/users/:id", {
      params: { id: RouteId },
      payload: UserChanges,
      success: ManagedAccount,
      error: [UserNotFound, OwnAccess, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.delete("remove", "/v1/users/:id", {
      params: { id: RouteId },
      success: HttpApiSchema.NoContent,
      error: [UserNotFound, OwnAccess, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("verifyEmail", "/v1/users/:id/verify", {
      params: { id: RouteId },
      success: ManagedAccount,
      error: [UserNotFound, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("grantLicence", "/v1/users/:id/licence", {
      params: { id: RouteId },
      payload: Schema.Struct({ months: Months }),
      success: ManagedAccount,
      error: [UserNotFound, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.delete("endLicence", "/v1/users/:id/licence", {
      params: { id: RouteId },
      success: ManagedAccount,
      error: [UserNotFound, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("extendTrial", "/v1/users/:id/trial", {
      params: { id: RouteId },
      payload: Schema.Struct({ days: Days }),
      success: ManagedAccount,
      error: [UserNotFound, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("setPassword", "/v1/users/:id/password", {
      params: { id: RouteId },
      payload: Schema.Struct({ password: NewPassword }),
      success: ManagedAccount,
      error: [UserNotFound, OwnAccess, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("sendReset", "/v1/users/:id/reset-email", {
      params: { id: RouteId },
      success: HttpApiSchema.Accepted,
      error: [UserNotFound, StorageFailure],
    }),
  )
  .add(
    HttpApiEndpoint.post("signOut", "/v1/users/:id/sign-out", {
      params: { id: RouteId },
      success: ManagedAccount,
      error: [UserNotFound, OwnAccess, StorageFailure],
    }),
  )
  .middleware(AdminsOnly)
  .middleware(Authentication) {}

export class HealthGroup extends HttpApiGroup.make("health").add(
  HttpApiEndpoint.get("health", "/health", {
    success: Schema.String.pipe(HttpApiSchema.asText()),
  }),
) {}

export class Api extends HttpApi.make("api")
  .add(AuthGroup)
  .add(AccountGroup)
  .add(CatalogGroup)
  .add(PublishingGroup)
  .add(TemplateAdminGroup)
  .add(UsersGroup)
  .add(HealthGroup) {}

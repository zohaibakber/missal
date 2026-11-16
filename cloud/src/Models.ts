import * as Schema from "effect/Schema";
import * as SchemaTransformation from "effect/SchemaTransformation";
import * as Model from "effect/schema/Model";

export const UserId = Schema.String.pipe(Schema.brand("UserId"));
export type UserId = typeof UserId.Type;

export const SessionId = Schema.String.pipe(Schema.brand("SessionId"));
export type SessionId = typeof SessionId.Type;

export const TemplateId = Schema.String.pipe(Schema.brand("TemplateId"));
export type TemplateId = typeof TemplateId.Type;

export const RouteId = Schema.String.check(Schema.isPattern(/^[A-Za-z0-9_-]{1,64}$/));

export const Role = Schema.Literals(["admin", "publisher", "member"]);
export type Role = typeof Role.Type;

export const canPublish = (role: Role) => role === "admin" || role === "publisher";

export const Email = Schema.Trim.pipe(
  Schema.decodeTo(
    Schema.String.check(
      Schema.isLowercased(),
      Schema.isMaxLength(254),
      Schema.isPattern(/^[^\s@]+@[^\s@]+\.[^\s@]+$/),
    ),
    SchemaTransformation.toLowerCase(),
  ),
);
export type Email = typeof Email.Type;

export const Name = Schema.String.check(Schema.isTrimmed(), Schema.isMaxLength(120));

export class User extends Model.Class<User>("User")({
  id: Model.UuidV4Insert(UserId),
  email: Email,
  name: Name,
  role: Role,
  passwordHash: Model.Sensitive(Schema.String),
  passwordSalt: Model.Sensitive(Schema.String),
  passwordIterations: Model.Sensitive(Schema.Int),
  mustChangePassword: Model.BooleanSqlite,
  disabled: Model.BooleanSqlite,
  createdAt: Model.DateTimeInsertFromNumber,
  updatedAt: Model.DateTimeUpdateFromNumber,
}) {}

export class Session extends Model.Class<Session>("Session")({
  id: Model.UuidV4Insert(SessionId),
  userId: UserId,
  refreshHash: Model.Sensitive(Schema.String),
  createdAt: Model.DateTimeInsertFromNumber,
  lastUsedAt: Model.DateTimeUpdateFromNumber,
  expiresAt: Schema.DateTimeUtcFromMillis,
}) {}

export class Template extends Model.Class<Template>("Template")({
  id: Model.UuidV4Insert(TemplateId),
  name: Schema.String,
  version: Schema.Int,
  fieldCount: Schema.Int,
  preview: Schema.String,
  size: Schema.Int,
  publishedBy: UserId,
  createdAt: Model.DateTimeInsertFromNumber,
  updatedAt: Model.DateTimeUpdateFromNumber,
}) {}

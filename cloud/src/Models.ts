import * as DateTime from "effect/DateTime";
import * as Option from "effect/Option";
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
  disabled: Model.BooleanSqlite,
  emailVerifiedAt: Model.FieldOption(Schema.DateTimeUtcFromMillis),
  trialEndsAt: Model.FieldOption(Schema.DateTimeUtcFromMillis),
  paidUntil: Model.FieldOption(Schema.DateTimeUtcFromMillis),
  createdAt: Model.DateTimeInsertFromNumber,
  updatedAt: Model.DateTimeUpdateFromNumber,
}) {}

export const CodePurpose = Schema.Literals(["verify", "reset"]);
export type CodePurpose = typeof CodePurpose.Type;

const licenceVariant = <const S extends string>(status: S) =>
  Schema.Struct({ status: Schema.Literal(status), until: Schema.DateTimeUtcFromMillis });

export const Licence = Schema.Union([
  licenceVariant("trial"),
  licenceVariant("licensed"),
  licenceVariant("expired"),
]);
export type Licence = typeof Licence.Type;

export const TRIAL_LENGTH = { days: 7 } as const;
const ADMIN_LICENCE_END = DateTime.makeUnsafe(Date.UTC(9999, 11, 31));

export const licenceOf = (
  user: Pick<User, "role" | "trialEndsAt" | "paidUntil">,
  now: DateTime.Utc,
): Licence => {
  if (user.role === "admin") return { status: "licensed", until: ADMIN_LICENCE_END };
  const future = (end: Option.Option<DateTime.Utc>) =>
    Option.filter(end, (until) => DateTime.isGreaterThan(until, now));
  const paid = future(user.paidUntil);
  if (Option.isSome(paid)) return { status: "licensed", until: paid.value };
  const trial = future(user.trialEndsAt);
  if (Option.isSome(trial)) return { status: "trial", until: trial.value };
  const ended = [user.trialEndsAt, user.paidUntil].flatMap(Option.toArray);
  return { status: "expired", until: ended.reduce((a, b) => DateTime.max(a, b), ended[0] ?? now) };
};

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

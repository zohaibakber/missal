import * as Context from "effect/Context";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlModel from "effect/sql/SqlModel";
import * as SqlSchema from "effect/sql/SqlSchema";
import { type Account, UserNotFound } from "./Api.ts";
import { storage } from "./Failures.ts";
import { licenceOf, TRIAL_LENGTH, User, UserId } from "./Models.ts";

export const millis = DateTime.toEpochMillis;

export const isVerified = (user: User) => Option.isSome(user.emailVerifiedAt);

export const toAccount = (user: User, now: DateTime.Utc): Account => ({
  id: user.id,
  email: user.email,
  name: user.name,
  role: user.role,
  disabled: user.disabled,
  emailVerified: isVerified(user),
  createdAt: user.createdAt,
  licence: licenceOf(user, now),
});

export interface UserFields {
  readonly name?: string;
  readonly role?: User["role"];
  readonly disabled?: boolean;
  readonly password?: string;
  readonly paidUntil?: DateTime.Utc;
  readonly trialEndsAt?: DateTime.Utc;
}

const missing = () => new UserNotFound({ error: "no such user" });

export class Users extends Context.Service<Users>()("Users", {
  make: Effect.gen(function* () {
    const sql = yield* SqlClient.SqlClient;
    const repository = yield* SqlModel.makeRepository(User, {
      tableName: "users",
      spanPrefix: "Users",
      idColumn: "id",
    });
    const encodeInsert = Schema.encodeEffect(User.insert);

    const findByEmail = SqlSchema.findOneOption({
      Request: Schema.String,
      Result: User,
      execute: (email) => sql`SELECT * FROM users WHERE email = ${email}`,
    });
    const findAll = SqlSchema.findAll({
      Request: Schema.Void,
      Result: User,
      execute: () => sql`SELECT * FROM users ORDER BY email`,
    });

    const find = (id: string) =>
      repository
        .findById(UserId.make(id))
        .pipe(Effect.catchNoSuchElement, storage("read the user"));

    const existing = (id: string) =>
      Effect.flatMap(find(id), (user) =>
        Option.isSome(user) ? Effect.succeed(user.value) : Effect.fail(missing()),
      );

    const byEmail = (email: string) => findByEmail(email).pipe(storage("read the user"));

    const all = findAll(undefined).pipe(storage("list users"));

    const insert = Effect.fnUntraced(function* (user: typeof User.insert.Type) {
      const row = yield* encodeInsert(user).pipe(Effect.orDie);
      const inserted = yield* sql`INSERT INTO users ${sql.insert(row)}
        ON CONFLICT (email) DO NOTHING RETURNING id`.pipe(storage("create the account"));
      return inserted.length > 0;
    });

    const update = (id: string, fields: UserFields, now: DateTime.Utc) =>
      sql`UPDATE users SET ${sql.update({
        ...(fields.name === undefined ? {} : { name: fields.name }),
        ...(fields.role === undefined ? {} : { role: fields.role }),
        ...(fields.disabled === undefined ? {} : { disabled: fields.disabled ? 1 : 0 }),
        ...(fields.password === undefined ? {} : { password: fields.password }),
        ...(fields.paidUntil === undefined ? {} : { paidUntil: millis(fields.paidUntil) }),
        ...(fields.trialEndsAt === undefined ? {} : { trialEndsAt: millis(fields.trialEndsAt) }),
        updatedAt: millis(now),
      })} WHERE id = ${id}`;

    const replaceUnverified = (id: string, name: string, password: string, now: DateTime.Utc) =>
      sql`UPDATE users SET ${sql.update({ name, password, updatedAt: millis(now) })}
        WHERE id = ${id} AND email_verified_at IS NULL`.pipe(storage("sign up"));

    const confirm = (id: string, now: DateTime.Utc) =>
      sql`UPDATE users SET
          email_verified_at = COALESCE(email_verified_at, ${millis(now)}),
          trial_ends_at = COALESCE(trial_ends_at, ${millis(DateTime.add(now, TRIAL_LENGTH))}),
          updated_at = ${millis(now)}
        WHERE id = ${id}`;

    const endLicence = (id: string, now: DateTime.Utc) =>
      sql`UPDATE users SET
          paid_until = CASE WHEN paid_until > ${millis(now)} THEN ${millis(now)} ELSE paid_until END,
          trial_ends_at = CASE WHEN trial_ends_at > ${millis(now)} THEN ${millis(now)} ELSE trial_ends_at END,
          updated_at = ${millis(now)}
        WHERE id = ${id}`;

    const remove = (id: string) => sql`DELETE FROM users WHERE id = ${id}`;

    const purgeUnverified = (createdBefore: DateTime.Utc) =>
      sql`DELETE FROM users WHERE email_verified_at IS NULL AND created_at <= ${millis(createdBefore)}`;

    return {
      find,
      existing,
      byEmail,
      all,
      insert,
      update,
      replaceUnverified,
      confirm,
      endLicence,
      remove,
      purgeUnverified,
    };
  }),
}) {
  static readonly layer = Layer.effect(Users, Users.make);
}

import { and, eq, isNull, lte, sql } from "drizzle-orm";
import * as Context from "effect/Context";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import { type Account, UserNotFound } from "./Api.ts";
import { Db } from "./Database.ts";
import { storage } from "./Failures.ts";
import { licenceOf, TRIAL_LENGTH, User } from "./Models.ts";
import { users } from "./Schema.ts";

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
  readonly name?: string | undefined;
  readonly role?: User["role"] | undefined;
  readonly disabled?: boolean | undefined;
  readonly password?: string | undefined;
  readonly paidUntil?: DateTime.Utc | undefined;
  readonly trialEndsAt?: DateTime.Utc | undefined;
}

const missing = () => new UserNotFound({ error: "no such user" });

const decodeUser = Schema.decodeUnknownEffect(User);
const decodeUsers = Schema.decodeUnknownEffect(Schema.Array(User));
const encodeInsert = Schema.encodeEffect(User.insert);

const optionalMillis = (time: DateTime.Utc | undefined) =>
  time === undefined ? undefined : millis(time);

export class Users extends Context.Service<Users>()("Users", {
  make: Effect.gen(function* () {
    const db = yield* Db;

    const first = (rows: ReadonlyArray<unknown>) =>
      rows[0] === undefined ? Effect.succeedNone : Effect.asSome(decodeUser(rows[0]));

    const find = (id: string) =>
      db
        .select()
        .from(users)
        .where(eq(users.id, id))
        .pipe(Effect.flatMap(first), storage("read the user"));

    const existing = (id: string) =>
      Effect.flatMap(find(id), (user) =>
        Option.isSome(user) ? Effect.succeed(user.value) : Effect.fail(missing()),
      );

    const byEmail = (email: string) =>
      db
        .select()
        .from(users)
        .where(eq(users.email, email))
        .pipe(Effect.flatMap(first), storage("read the user"));

    const all = db
      .select()
      .from(users)
      .orderBy(users.email)
      .pipe(Effect.flatMap(decodeUsers), storage("list users"));

    const insert = Effect.fnUntraced(function* (user: typeof User.insert.Type) {
      const row = yield* encodeInsert(user).pipe(Effect.orDie);
      const inserted = yield* db
        .insert(users)
        .values(row)
        .onConflictDoNothing({ target: users.email })
        .returning({ id: users.id })
        .pipe(storage("create the account"));
      return inserted.length > 0;
    });

    const update = (id: string, fields: UserFields, now: DateTime.Utc) =>
      db
        .update(users)
        .set({
          name: fields.name,
          role: fields.role,
          disabled: fields.disabled === undefined ? undefined : fields.disabled ? 1 : 0,
          password: fields.password,
          paidUntil: optionalMillis(fields.paidUntil),
          trialEndsAt: optionalMillis(fields.trialEndsAt),
          updatedAt: millis(now),
        })
        .where(eq(users.id, id));

    const replaceUnverified = (id: string, name: string, password: string, now: DateTime.Utc) =>
      db
        .update(users)
        .set({ name, password, updatedAt: millis(now) })
        .where(and(eq(users.id, id), isNull(users.emailVerifiedAt)))
        .pipe(storage("sign up"));

    const confirm = (id: string, now: DateTime.Utc) =>
      db
        .update(users)
        .set({
          emailVerifiedAt: sql`COALESCE(${users.emailVerifiedAt}, ${millis(now)})`,
          trialEndsAt: sql`COALESCE(${users.trialEndsAt}, ${millis(DateTime.add(now, TRIAL_LENGTH))})`,
          updatedAt: millis(now),
        })
        .where(eq(users.id, id));

    const endLicence = (id: string, now: DateTime.Utc) => {
      const at = millis(now);
      return db
        .update(users)
        .set({
          paidUntil: sql`CASE WHEN ${users.paidUntil} > ${at} THEN ${at} ELSE ${users.paidUntil} END`,
          trialEndsAt: sql`CASE WHEN ${users.trialEndsAt} > ${at} THEN ${at} ELSE ${users.trialEndsAt} END`,
          updatedAt: at,
        })
        .where(eq(users.id, id));
    };

    const remove = (id: string) => db.delete(users).where(eq(users.id, id));

    const purgeUnverified = (createdBefore: DateTime.Utc) =>
      db
        .delete(users)
        .where(and(isNull(users.emailVerifiedAt), lte(users.createdAt, millis(createdBefore))));

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

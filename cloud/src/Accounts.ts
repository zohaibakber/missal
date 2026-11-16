import type { RuntimeContext } from "alchemy/RuntimeContext";
import * as Context from "effect/Context";
import * as Digest from "effect/crypto/Digest";
import * as Mac from "effect/crypto/Mac";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as DateTime from "effect/DateTime";
import * as Duration from "effect/Duration";
import * as Effect from "effect/Effect";
import * as Base64Url from "effect/encoding/Base64Url";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Redacted from "effect/Redacted";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlModel from "effect/sql/SqlModel";
import * as SqlSchema from "effect/sql/SqlSchema";
import {
  type Account,
  AccountLocked,
  EmailTaken,
  InvalidCredentials,
  type NewUser,
  OwnAccess,
  type Session as SessionTokens,
  SessionExpired,
  type StorageFailure,
  type UserChanges,
  UserNotFound,
  type Viewer,
  WrongPassword,
} from "./Api.ts";
import { Atomic } from "./Database.ts";
import { storage } from "./Failures.ts";
import { type Email, type Role, Session, SessionId, User, UserId } from "./Models.ts";
import { Passwords, type StoredPassword } from "./Passwords.ts";
import { SignInThrottle } from "./Throttle.ts";
import { ACCESS_SECONDS, Tokens } from "./Tokens.ts";

const SESSION_LIFETIME = Duration.days(60);
const LOCK_LIFETIME = Duration.minutes(15);
const MAX_FAILURES = 5;
const SECRET_BYTES = 32;

const FailureRow = Schema.Struct({ failures: Schema.Int, lockedUntil: Schema.Number });
const CountRow = Schema.Struct({ count: Schema.Int });

const encoder = new TextEncoder();

const toAccount = (user: User): Account => user;

const storedPassword = (user: User): StoredPassword => ({
  hash: user.passwordHash,
  salt: user.passwordSalt,
  iterations: user.passwordIterations,
});

const sameText = (left: string, right: string) => {
  const a = encoder.encode(left);
  const b = encoder.encode(right);
  return a.length === b.length && Mac.timingSafeEqual(a, b);
};

const splitToken = (token: string) => {
  const dot = token.indexOf(".");
  return dot > 0 && dot < token.length - 1
    ? Option.some({ sessionId: SessionId.make(token.slice(0, dot)), secret: token.slice(dot + 1) })
    : Option.none();
};

const millis = DateTime.toEpochMillis;

const expired = () => new SessionExpired({ error: "your session has ended; sign in again" });
const invalid = () => new InvalidCredentials({ error: "wrong email or password" });
const missing = () => new UserNotFound({ error: "no such user" });

export interface Bootstrap {
  readonly email: string;
  readonly password: Redacted.Redacted<string>;
}

type Effectful<A, E> = Effect.Effect<A, E, RuntimeContext>;

export class Accounts extends Context.Service<
  Accounts,
  {
    readonly signIn: (
      client: string,
      email: Email,
      password: Redacted.Redacted<string>,
    ) => Effectful<SessionTokens, InvalidCredentials | AccountLocked | StorageFailure>;
    readonly refresh: (token: string) => Effectful<SessionTokens, SessionExpired | StorageFailure>;
    readonly signOut: (token: string) => Effectful<void, StorageFailure>;
    readonly me: (viewer: Viewer["Service"]) => Effectful<Account, UserNotFound | StorageFailure>;
    readonly changePassword: (
      viewer: Viewer["Service"],
      current: Redacted.Redacted<string>,
      next: Redacted.Redacted<string>,
    ) => Effectful<SessionTokens, WrongPassword | UserNotFound | StorageFailure>;
    readonly list: Effectful<ReadonlyArray<Account>, StorageFailure>;
    readonly create: (user: NewUser) => Effectful<Account, EmailTaken | StorageFailure>;
    readonly update: (
      viewer: Viewer["Service"],
      id: string,
      changes: UserChanges,
    ) => Effectful<Account, UserNotFound | OwnAccess | StorageFailure>;
    readonly resetPassword: (
      id: string,
      password: Redacted.Redacted<string>,
    ) => Effectful<Account, UserNotFound | StorageFailure>;
    readonly purgeExpired: Effectful<void, StorageFailure>;
  }
>()("missal/Accounts") {
  static readonly layer = (bootstrap: Bootstrap) =>
    Layer.effect(
      Accounts,
      Effect.gen(function* () {
        const sql = yield* SqlClient.SqlClient;
        const atomic = yield* Atomic;
        const passwords = yield* Passwords;
        const tokens = yield* Tokens;
        const throttle = yield* SignInThrottle;
        const random = yield* SecureRandom.SecureRandom;
        const digest = yield* Digest.Digest;

        const users = yield* SqlModel.makeRepository(User, {
          tableName: "users",
          spanPrefix: "Users",
          idColumn: "id",
        });
        const sessions = yield* SqlModel.makeRepository(Session, {
          tableName: "sessions",
          spanPrefix: "Sessions",
          idColumn: "id",
        });
        const encodeSession = Schema.encodeEffect(Session.insert);

        const userByEmail = SqlSchema.findOneOption({
          Request: Schema.String,
          Result: User,
          execute: (email) => sql`SELECT * FROM users WHERE email = ${email}`,
        });
        const allUsers = SqlSchema.findAll({
          Request: Schema.Void,
          Result: User,
          execute: () => sql`SELECT * FROM users ORDER BY email`,
        });
        const insertUser = SqlSchema.findOneOption({
          Request: User.insert,
          Result: User,
          execute: (row) =>
            sql`INSERT INTO users ${sql.insert(row)} ON CONFLICT (email) DO NOTHING RETURNING *`,
        });
        const failuresFor = SqlSchema.findOneOption({
          Request: Schema.String,
          Result: FailureRow,
          execute: (email) =>
            sql`SELECT failures, locked_until FROM sign_in_failures WHERE email = ${email}`,
        });
        const userCount = SqlSchema.findOneOption({
          Request: Schema.Void,
          Result: CountRow,
          execute: () => sql`SELECT COUNT(*) AS count FROM users`,
        });

        const findUser = (id: string) =>
          users.findById(UserId.make(id)).pipe(
            Effect.map(Option.some),
            Effect.catchTag("NoSuchElementError", () => Effect.succeedNone),
          );

        const existing = (id: string) =>
          findUser(id).pipe(
            storage("read the user"),
            Effect.flatMap((user) =>
              Option.isSome(user) ? Effect.succeed(user.value) : Effect.fail(missing()),
            ),
          );

        const hashSecret = (secret: string) =>
          digest.digest(Digest.sha256, encoder.encode(secret)).pipe(Effect.map(Base64Url.encode));

        const newSecret = random.randomBytes(SECRET_BYTES).pipe(Effect.map(Base64Url.encode));

        const issue = Effect.fnUntraced(function* (user: User, sessionId: SessionId, secret: string) {
          const accessToken = yield* tokens.issue({
            userId: user.id,
            sessionId,
            role: user.role,
            mustChangePassword: user.mustChangePassword,
          });
          return {
            accessToken,
            refreshToken: `${sessionId}.${secret}`,
            expiresIn: ACCESS_SECONDS,
            account: toAccount(user),
          } satisfies SessionTokens;
        });

        const newSession = Effect.fnUntraced(function* (user: User) {
          const secret = yield* newSecret;
          const now = yield* DateTime.now;
          const session = Session.insert.make({
            id: SessionId.make(yield* random.uuidV4()),
            userId: user.id,
            refreshHash: yield* hashSecret(secret),
            expiresAt: DateTime.addDuration(now, SESSION_LIFETIME),
          });
          return { session, secret };
        });

        const openSession = Effect.fnUntraced(function* (user: User) {
          const { session, secret } = yield* newSession(user);
          const stored = yield* sessions.insert(session);
          return yield* issue(user, stored.id, secret);
        });

        const passwordFields = Effect.fnUntraced(function* (
          password: Redacted.Redacted<string>,
          mustChange: boolean,
        ) {
          const stored = yield* passwords.hash(password);
          return {
            password_hash: stored.hash,
            password_salt: stored.salt,
            password_iterations: stored.iterations,
            must_change_password: mustChange ? 1 : 0,
            updated_at: millis(yield* DateTime.now),
          };
        });

        const createUser = Effect.fnUntraced(function* (
          fields: { email: Email; name: string; role: Role },
          password: Redacted.Redacted<string>,
        ) {
          const stored = yield* passwords.hash(password);
          return yield* insertUser(
            User.insert.make({
              ...fields,
              id: UserId.make(yield* random.uuidV4()),
              passwordHash: stored.hash,
              passwordSalt: stored.salt,
              passwordIterations: stored.iterations,
              mustChangePassword: true,
              disabled: false,
            }),
          );
        });

        const bootstrapAdmin = Effect.fnUntraced(function* (
          email: Email,
          password: Redacted.Redacted<string>,
        ) {
          if (email !== bootstrap.email.trim().toLowerCase()) return Option.none<User>();
          if (!sameText(Redacted.value(password), Redacted.value(bootstrap.password))) {
            return Option.none<User>();
          }
          const count = yield* userCount(undefined);
          if (Option.isSome(count) && count.value.count > 0) return Option.none<User>();
          yield* Effect.logInfo(`creating the first administrator, ${email}`);
          return yield* createUser({ email, name: "", role: "admin" }, password);
        });

        const recordFailure = (email: string, now: number) =>
          sql`INSERT INTO sign_in_failures (email, failures, locked_until) VALUES (${email}, 1, 0)
            ON CONFLICT (email) DO UPDATE SET
              failures = CASE WHEN failures + 1 >= ${MAX_FAILURES} THEN 0 ELSE failures + 1 END,
              locked_until = CASE WHEN failures + 1 >= ${MAX_FAILURES} THEN ${now + Duration.toMillis(LOCK_LIFETIME)} ELSE locked_until END`;

        const signIn = Effect.fn("Accounts.signIn")(function* (
          client: string,
          email: Email,
          password: Redacted.Redacted<string>,
        ) {
          yield* throttle.check(client);
          const now = millis(yield* DateTime.now);
          const lock = yield* failuresFor(email).pipe(storage("check sign-in attempts"));
          if (Option.isSome(lock) && lock.value.lockedUntil > now) {
            return yield* new AccountLocked({
              error: "too many failed sign-ins; try again later",
              retryAfter: Math.ceil((lock.value.lockedUntil - now) / 1000),
            });
          }
          const found = yield* userByEmail(email).pipe(
            Effect.flatMap((user) =>
              Option.isSome(user) ? Effect.succeed(user) : bootstrapAdmin(email, password),
            ),
            storage("sign in"),
          );
          const stored = Option.isSome(found) ? storedPassword(found.value) : yield* passwords.decoy;
          const matches = yield* passwords.verify(password, stored);
          if (Option.isNone(found) || !matches || found.value.disabled) {
            yield* recordFailure(email, now).pipe(storage("record a failed sign-in"));
            return yield* invalid();
          }
          yield* sql`DELETE FROM sign_in_failures WHERE email = ${email}`.pipe(storage("sign in"));
          return yield* openSession(found.value).pipe(storage("open a session"));
        });

        const refresh = Effect.fn("Accounts.refresh")(function* (token: string) {
          const parts = splitToken(token);
          if (Option.isNone(parts)) return yield* expired();
          const { sessionId, secret } = parts.value;
          const now = yield* DateTime.now;
          const session = yield* sessions.findById(sessionId).pipe(
            Effect.map(Option.some),
            Effect.catchTag("NoSuchElementError", () => Effect.succeedNone),
            storage("refresh the session"),
          );
          if (Option.isNone(session)) return yield* expired();
          const user = yield* findUser(session.value.userId).pipe(storage("refresh the session"));
          const presented = yield* hashSecret(secret);
          const valid =
            Option.isSome(user) &&
            !user.value.disabled &&
            DateTime.isGreaterThan(session.value.expiresAt, now) &&
            sameText(presented, session.value.refreshHash);
          if (!valid || Option.isNone(user)) {
            yield* sessions.delete(sessionId).pipe(storage("end the session"));
            return yield* expired();
          }
          const next = yield* newSecret;
          const rotated = yield* sql`UPDATE sessions SET ${sql.update({
            refreshHash: yield* hashSecret(next),
            lastUsedAt: millis(now),
            expiresAt: millis(DateTime.addDuration(now, SESSION_LIFETIME)),
          })} WHERE id = ${sessionId} AND refresh_hash = ${session.value.refreshHash} RETURNING id`.pipe(
            storage("refresh the session"),
          );
          if (rotated.length === 0) return yield* expired();
          return yield* issue(user.value, sessionId, next);
        });

        const signOut = Effect.fn("Accounts.signOut")(function* (token: string) {
          const parts = splitToken(token);
          if (Option.isNone(parts)) return;
          const presented = yield* hashSecret(parts.value.secret);
          yield* sql`DELETE FROM sessions WHERE id = ${parts.value.sessionId} AND refresh_hash = ${presented}`.pipe(
            storage("sign out"),
          );
        });

        const me = Effect.fn("Accounts.me")(function* (viewer: Viewer["Service"]) {
          return toAccount(yield* existing(viewer.id));
        });

        const changePassword = Effect.fn("Accounts.changePassword")(function* (
          viewer: Viewer["Service"],
          current: Redacted.Redacted<string>,
          next: Redacted.Redacted<string>,
        ) {
          const user = yield* existing(viewer.id);
          if (!(yield* passwords.verify(current, storedPassword(user)))) {
            return yield* new WrongPassword({ error: "the current password is wrong" });
          }
          const fields = yield* passwordFields(next, false);
          const { session, secret } = yield* newSession(user);
          const row = yield* encodeSession(session).pipe(Effect.orDie);
          yield* atomic
            .run([
              sql`UPDATE users SET ${sql.update(fields)} WHERE id = ${user.id}`,
              sql`DELETE FROM sessions WHERE user_id = ${user.id}`,
              sql`INSERT INTO sessions ${sql.insert(row)}`,
            ])
            .pipe(storage("change the password"));
          const updated = yield* existing(user.id);
          return yield* issue(updated, session.id, secret);
        });

        const list = allUsers(undefined).pipe(
          Effect.map((rows) => rows.map(toAccount)),
          storage("list users"),
          Effect.withSpan("Accounts.list"),
        );

        const create = Effect.fn("Accounts.create")(function* (user: NewUser) {
          const created = yield* createUser(
            { email: user.email, name: user.name, role: user.role },
            user.password,
          ).pipe(storage("create the user"));
          if (Option.isNone(created)) {
            return yield* new EmailTaken({ error: "an account with this email already exists" });
          }
          return toAccount(created.value);
        });

        const update = Effect.fn("Accounts.update")(function* (
          viewer: Viewer["Service"],
          id: string,
          changes: UserChanges,
        ) {
          const demotesSelf =
            id === viewer.id &&
            ((changes.role !== undefined && changes.role !== "admin") || changes.disabled === true);
          if (demotesSelf) {
            return yield* new OwnAccess({ error: "you cannot remove your own administrator access" });
          }
          const user = yield* existing(id);
          const fields = {
            ...(changes.name === undefined ? {} : { name: changes.name }),
            ...(changes.role === undefined ? {} : { role: changes.role }),
            ...(changes.disabled === undefined ? {} : { disabled: changes.disabled ? 1 : 0 }),
            updated_at: millis(yield* DateTime.now),
          };
          yield* atomic
            .run([
              sql`UPDATE users SET ${sql.update(fields)} WHERE id = ${user.id}`,
              ...(changes.disabled === true
                ? [sql`DELETE FROM sessions WHERE user_id = ${user.id}`]
                : []),
            ])
            .pipe(storage("update the user"));
          return toAccount(yield* existing(user.id));
        });

        const resetPassword = Effect.fn("Accounts.resetPassword")(function* (
          id: string,
          password: Redacted.Redacted<string>,
        ) {
          const user = yield* existing(id);
          const fields = yield* passwordFields(password, true);
          yield* atomic
            .run([
              sql`UPDATE users SET ${sql.update(fields)} WHERE id = ${user.id}`,
              sql`DELETE FROM sessions WHERE user_id = ${user.id}`,
            ])
            .pipe(storage("reset the password"));
          return toAccount(yield* existing(user.id));
        });

        const purgeExpired = Effect.gen(function* () {
          const now = millis(yield* DateTime.now);
          yield* atomic.run([
            sql`DELETE FROM sessions WHERE expires_at <= ${now}`,
            sql`DELETE FROM sign_in_failures WHERE locked_until <= ${now - Duration.toMillis(Duration.days(1))}`,
          ]);
        }).pipe(storage("purge expired sessions"), Effect.withSpan("Accounts.purgeExpired"));

        return Accounts.of({
          signIn,
          refresh,
          signOut,
          me,
          changePassword,
          list,
          create,
          update,
          resetPassword,
          purgeExpired,
        });
      }),
    );
}

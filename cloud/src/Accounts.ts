import * as Clock from "effect/Clock";
import * as Context from "effect/Context";
import * as Digest from "effect/crypto/Digest";
import * as Mac from "effect/crypto/Mac";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as Effect from "effect/Effect";
import * as Base64Url from "effect/encoding/Base64Url";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Redacted from "effect/Redacted";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlSchema from "effect/sql/SqlSchema";
import {
  type Account,
  AccountLocked,
  EmailTaken,
  InvalidCredentials,
  type NewUser,
  OwnAccess,
  Role,
  type Session,
  SessionExpired,
  type StorageFailure,
  type UserChanges,
  UserNotFound,
  type Viewer,
  WrongPassword,
} from "./Api.ts";
import { storage } from "./Failures.ts";
import { Passwords, type StoredPassword } from "./Passwords.ts";
import { ACCESS_SECONDS, Tokens } from "./Tokens.ts";

const SESSION_MS = 60 * 24 * 60 * 60 * 1000;
const LOCK_MS = 15 * 60 * 1000;
const MAX_FAILURES = 5;
const SECRET_BYTES = 32;

const UserRow = Schema.Struct({
  id: Schema.String,
  email: Schema.String,
  name: Schema.String,
  role: Role,
  password_hash: Schema.String,
  password_salt: Schema.String,
  password_iterations: Schema.Int,
  must_change_password: Schema.Int,
  disabled: Schema.Int,
  created_at: Schema.Number,
});
type UserRow = typeof UserRow.Type;

const SessionRow = Schema.Struct({
  id: Schema.String,
  refresh_hash: Schema.String,
  expires_at: Schema.Number,
  user_id: Schema.String,
});

const FailureRow = Schema.Struct({ failures: Schema.Int, locked_until: Schema.Number });
const CountRow = Schema.Struct({ count: Schema.Int });

const encoder = new TextEncoder();

const toAccount = (row: UserRow): Account => ({
  id: row.id,
  email: row.email,
  name: row.name,
  role: row.role,
  mustChangePassword: row.must_change_password === 1,
  disabled: row.disabled === 1,
  createdAt: row.created_at,
});

const storedPassword = (row: UserRow): StoredPassword => ({
  hash: row.password_hash,
  salt: row.password_salt,
  iterations: row.password_iterations,
});

const sameText = (left: string, right: string) => {
  const a = encoder.encode(left);
  const b = encoder.encode(right);
  return a.length === b.length && Mac.timingSafeEqual(a, b);
};

const splitToken = (token: string) => {
  const dot = token.indexOf(".");
  return dot > 0 && dot < token.length - 1
    ? Option.some({ sessionId: token.slice(0, dot), secret: token.slice(dot + 1) })
    : Option.none();
};

const normalize = (email: string) => email.trim().toLowerCase();

const expired = () => new SessionExpired({ error: "your session has ended; sign in again" });
const invalid = () => new InvalidCredentials({ error: "wrong email or password" });
const missing = () => new UserNotFound({ error: "no such user" });

export interface Bootstrap {
  readonly email: string;
  readonly password: Redacted.Redacted<string>;
}

export class Accounts extends Context.Service<
  Accounts,
  {
    readonly signIn: (
      email: string,
      password: Redacted.Redacted<string>,
    ) => Effect.Effect<Session, InvalidCredentials | AccountLocked | StorageFailure>;
    readonly refresh: (token: string) => Effect.Effect<Session, SessionExpired | StorageFailure>;
    readonly signOut: (token: string) => Effect.Effect<void, StorageFailure>;
    readonly me: (viewer: Viewer["Service"]) => Effect.Effect<Account, UserNotFound | StorageFailure>;
    readonly changePassword: (
      viewer: Viewer["Service"],
      current: Redacted.Redacted<string>,
      next: Redacted.Redacted<string>,
    ) => Effect.Effect<Session, WrongPassword | UserNotFound | StorageFailure>;
    readonly list: Effect.Effect<ReadonlyArray<Account>, StorageFailure>;
    readonly create: (user: NewUser) => Effect.Effect<Account, EmailTaken | StorageFailure>;
    readonly update: (
      viewer: Viewer["Service"],
      id: string,
      changes: UserChanges,
    ) => Effect.Effect<Account, UserNotFound | OwnAccess | StorageFailure>;
    readonly resetPassword: (
      id: string,
      password: Redacted.Redacted<string>,
    ) => Effect.Effect<Account, UserNotFound | StorageFailure>;
  }
>()("missal/Accounts") {
  static readonly layer = (bootstrap: Bootstrap) =>
    Layer.effect(
      Accounts,
      Effect.gen(function* () {
        const sql = yield* SqlClient.SqlClient;
        const passwords = yield* Passwords;
        const tokens = yield* Tokens;
        const random = yield* SecureRandom.SecureRandom;
        const digest = yield* Digest.Digest;

        const userByEmail = SqlSchema.findOneOption({
          Request: Schema.String,
          Result: UserRow,
          execute: (email) => sql`SELECT * FROM users WHERE email = ${email}`,
        });
        const userById = SqlSchema.findOneOption({
          Request: Schema.String,
          Result: UserRow,
          execute: (id) => sql`SELECT * FROM users WHERE id = ${id}`,
        });
        const allUsers = SqlSchema.findAll({
          Request: Schema.Void,
          Result: UserRow,
          execute: () => sql`SELECT * FROM users ORDER BY email`,
        });
        const sessionById = SqlSchema.findOneOption({
          Request: Schema.String,
          Result: SessionRow,
          execute: (id) =>
            sql`SELECT id, refresh_hash, expires_at, user_id FROM sessions WHERE id = ${id}`,
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

        const hashSecret = (secret: string) =>
          digest.digest(Digest.sha256, encoder.encode(secret)).pipe(Effect.map(Base64Url.encode));

        const newSecret = random.randomBytes(SECRET_BYTES).pipe(Effect.map(Base64Url.encode));

        const issue = Effect.fnUntraced(function* (user: UserRow, sessionId: string, secret: string) {
          const accessToken = yield* tokens.issue({
            userId: user.id,
            sessionId,
            role: user.role,
            mustChangePassword: user.must_change_password === 1,
          });
          return {
            accessToken,
            refreshToken: `${sessionId}.${secret}`,
            expiresIn: ACCESS_SECONDS,
            account: toAccount(user),
          } satisfies Session;
        });

        const openSession = Effect.fnUntraced(function* (user: UserRow) {
          const now = yield* Clock.currentTimeMillis;
          const id = yield* random.uuidV4();
          const secret = yield* newSecret;
          yield* sql`INSERT INTO sessions ${sql.insert({
            id,
            user_id: user.id,
            refresh_hash: yield* hashSecret(secret),
            created_at: now,
            last_used_at: now,
            expires_at: now + SESSION_MS,
          })}`;
          return yield* issue(user, id, secret);
        });

        const insertUser = Effect.fnUntraced(function* (
          fields: { email: string; name: string; role: Role },
          password: Redacted.Redacted<string>,
        ) {
          const now = yield* Clock.currentTimeMillis;
          const id = yield* random.uuidV4();
          const stored = yield* passwords.hash(password);
          yield* sql`INSERT INTO users ${sql.insert({
            id,
            email: fields.email,
            name: fields.name,
            role: fields.role,
            password_hash: stored.hash,
            password_salt: stored.salt,
            password_iterations: stored.iterations,
            must_change_password: 1,
            disabled: 0,
            created_at: now,
            updated_at: now,
          })}`;
          return yield* userById(id).pipe(Effect.flatMap(Effect.fromOption));
        });

        const bootstrapAdmin = Effect.fnUntraced(function* (
          email: string,
          password: Redacted.Redacted<string>,
        ) {
          if (email !== normalize(bootstrap.email)) return Option.none<UserRow>();
          if (!sameText(Redacted.value(password), Redacted.value(bootstrap.password))) {
            return Option.none<UserRow>();
          }
          const count = yield* userCount(undefined);
          if (Option.isSome(count) && count.value.count > 0) return Option.none<UserRow>();
          yield* Effect.logInfo(`creating the first administrator, ${email}`);
          return Option.some(yield* insertUser({ email, name: "", role: "admin" }, password));
        });

        const recordFailure = (email: string, now: number) =>
          sql`INSERT INTO sign_in_failures (email, failures, locked_until) VALUES (${email}, 1, 0)
            ON CONFLICT (email) DO UPDATE SET
              failures = CASE WHEN failures + 1 >= ${MAX_FAILURES} THEN 0 ELSE failures + 1 END,
              locked_until = CASE WHEN failures + 1 >= ${MAX_FAILURES} THEN ${now + LOCK_MS} ELSE locked_until END`;

        const signIn = Effect.fn("Accounts.signIn")(function* (
          input: string,
          password: Redacted.Redacted<string>,
        ) {
          const email = normalize(input);
          const now = yield* Clock.currentTimeMillis;
          const lock = yield* failuresFor(email).pipe(storage("check sign-in attempts"));
          if (Option.isSome(lock) && lock.value.locked_until > now) {
            return yield* new AccountLocked({
              error: "too many failed sign-ins; try again later",
              retryAfter: Math.ceil((lock.value.locked_until - now) / 1000),
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
          if (Option.isNone(found) || !matches || found.value.disabled === 1) {
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
          const now = yield* Clock.currentTimeMillis;
          const session = yield* sessionById(sessionId).pipe(storage("refresh the session"));
          if (Option.isNone(session)) return yield* expired();
          const user = yield* userById(session.value.user_id).pipe(storage("refresh the session"));
          const presented = yield* hashSecret(secret);
          const valid =
            Option.isSome(user) &&
            user.value.disabled === 0 &&
            session.value.expires_at > now &&
            sameText(presented, session.value.refresh_hash);
          if (!valid || Option.isNone(user)) {
            yield* sql`DELETE FROM sessions WHERE id = ${sessionId}`.pipe(
              storage("end the session"),
            );
            return yield* expired();
          }
          const next = yield* newSecret;
          const rotated = yield* sql`UPDATE sessions SET ${sql.update({
            refresh_hash: yield* hashSecret(next),
            last_used_at: now,
            expires_at: now + SESSION_MS,
          })} WHERE id = ${sessionId} AND refresh_hash = ${session.value.refresh_hash} RETURNING id`.pipe(
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

        const existing = (id: string) =>
          userById(id).pipe(
            storage("read the user"),
            Effect.flatMap((user) =>
              Option.isSome(user) ? Effect.succeed(user.value) : Effect.fail(missing()),
            ),
          );

        const me = Effect.fn("Accounts.me")(function* (viewer: Viewer["Service"]) {
          return toAccount(yield* existing(viewer.id));
        });

        const writePassword = Effect.fnUntraced(function* (
          id: string,
          password: Redacted.Redacted<string>,
          mustChange: boolean,
        ) {
          const stored = yield* passwords.hash(password);
          const now = yield* Clock.currentTimeMillis;
          yield* sql`UPDATE users SET ${sql.update({
            password_hash: stored.hash,
            password_salt: stored.salt,
            password_iterations: stored.iterations,
            must_change_password: mustChange ? 1 : 0,
            updated_at: now,
          })} WHERE id = ${id}`;
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
          yield* writePassword(user.id, next, false).pipe(storage("change the password"));
          yield* sql`DELETE FROM sessions WHERE user_id = ${user.id}`.pipe(
            storage("change the password"),
          );
          const updated = yield* existing(user.id);
          return yield* openSession(updated).pipe(storage("open a session"));
        });

        const list = allUsers(undefined).pipe(
          Effect.map((rows) => rows.map(toAccount)),
          storage("list users"),
          Effect.withSpan("Accounts.list"),
        );

        const create = Effect.fn("Accounts.create")(function* (user: NewUser) {
          const email = normalize(user.email);
          const taken = yield* userByEmail(email).pipe(storage("create the user"));
          if (Option.isSome(taken)) {
            return yield* new EmailTaken({ error: "an account with this email already exists" });
          }
          const row = yield* insertUser({ email, name: user.name, role: user.role }, user.password).pipe(
            storage("create the user"),
          );
          return toAccount(row);
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
            return yield* new OwnAccess({
              error: "you cannot remove your own administrator access",
            });
          }
          yield* existing(id);
          const now = yield* Clock.currentTimeMillis;
          const fields = {
            ...(changes.name === undefined ? {} : { name: changes.name }),
            ...(changes.role === undefined ? {} : { role: changes.role }),
            ...(changes.disabled === undefined ? {} : { disabled: changes.disabled ? 1 : 0 }),
            updated_at: now,
          };
          yield* sql`UPDATE users SET ${sql.update(fields)} WHERE id = ${id}`.pipe(
            storage("update the user"),
          );
          if (changes.disabled === true) {
            yield* sql`DELETE FROM sessions WHERE user_id = ${id}`.pipe(storage("update the user"));
          }
          return toAccount(yield* existing(id));
        });

        const resetPassword = Effect.fn("Accounts.resetPassword")(function* (
          id: string,
          password: Redacted.Redacted<string>,
        ) {
          yield* existing(id);
          yield* writePassword(id, password, true).pipe(storage("reset the password"));
          yield* sql`DELETE FROM sessions WHERE user_id = ${id}`.pipe(storage("reset the password"));
          return toAccount(yield* existing(id));
        });

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
        });
      }),
    );
}

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
import type * as Redacted from "effect/Redacted";
import * as Schema from "effect/Schema";
import * as Model from "effect/schema/Model";
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlModel from "effect/sql/SqlModel";
import * as SqlSchema from "effect/sql/SqlSchema";
import {
  type Account,
  AccountLocked,
  EmailNotVerified,
  InvalidCode,
  InvalidCredentials,
  OwnAccess,
  type Session as SessionTokens,
  SessionExpired,
  type SignUp,
  type StorageFailure,
  type UserChanges,
  UserNotFound,
  type Viewer,
  WrongPassword,
} from "./Api.ts";
import { Atomic } from "./Database.ts";
import { storage } from "./Failures.ts";
import { type MailMessage, Mailer } from "./Mailer.ts";
import {
  type CodePurpose,
  type Email,
  licenceOf,
  Session,
  SessionId,
  TRIAL_LENGTH,
  User,
  UserId,
} from "./Models.ts";
import { Passwords, type StoredPassword } from "./Passwords.ts";
import { AuthThrottle } from "./Throttle.ts";
import { ACCESS_SECONDS, Tokens } from "./Tokens.ts";

const SESSION_LIFETIME = Duration.days(60);
const LOCK_LIFETIME = Duration.minutes(15);
const MAX_FAILURES = 5;
const SECRET_BYTES = 32;
const CODE_LIFETIME = Duration.minutes(10);
const CODE_COOLDOWN = Duration.seconds(60);
const MAX_CODE_ATTEMPTS = 5;
const UNVERIFIED_LIFETIME = Duration.days(7);

const FailureRow = Schema.Struct({ failures: Schema.Int, lockedUntil: Schema.Number });
const CodeRow = Schema.Struct({ codeHash: Schema.String, expiresAt: Schema.Number });
const CodeKey = Schema.Struct({ email: Schema.String, purpose: Schema.String });

const encoder = new TextEncoder();

const toAccount = (user: User, now: DateTime.Utc): Account => ({
  id: user.id,
  email: user.email,
  name: user.name,
  role: user.role,
  disabled: user.disabled,
  createdAt: user.createdAt,
  licence: licenceOf(user, now),
});

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

const isVerified = (user: User) => Option.isSome(user.emailVerifiedAt);

const expired = () => new SessionExpired({ error: "your session has ended; sign in again" });
const invalid = () => new InvalidCredentials({ error: "wrong email or password" });
const missing = () => new UserNotFound({ error: "no such user" });
const wrongCode = () => new InvalidCode({ error: "that code is wrong or has expired" });

const codeMessage = (to: Email, purpose: CodePurpose, code: string): MailMessage => ({
  to,
  subject: purpose === "verify" ? "Confirm your email for Missal" : "Reset your Missal password",
  text: [
    purpose === "verify"
      ? `Your code to confirm your email for Missal is ${code}.`
      : `Your code to reset your Missal password is ${code}.`,
    "It expires in 10 minutes.",
    "If you did not ask for this code, ignore this email.",
  ].join("\n\n"),
});

const accountExistsMessage = (to: Email): MailMessage => ({
  to,
  subject: "Your Missal account",
  text: [
    "Someone tried to create a Missal account with this email address, but it already has one.",
    "If that was you, sign in, or choose Forgot password on the sign-in screen.",
    "If it was not you, ignore this email.",
  ].join("\n\n"),
});

type Effectful<A, E> = Effect.Effect<A, E, RuntimeContext>;

export class Accounts extends Context.Service<
  Accounts,
  {
    readonly signUp: (
      client: string,
      request: SignUp,
    ) => Effectful<void, AccountLocked | StorageFailure>;
    readonly requestCode: (
      client: string,
      email: Email,
      purpose: CodePurpose,
    ) => Effectful<void, AccountLocked | StorageFailure>;
    readonly verifyEmail: (
      client: string,
      email: Email,
      code: string,
    ) => Effectful<SessionTokens, InvalidCode | AccountLocked | StorageFailure>;
    readonly resetPassword: (
      client: string,
      email: Email,
      code: string,
      password: Redacted.Redacted<string>,
    ) => Effectful<SessionTokens, InvalidCode | AccountLocked | StorageFailure>;
    readonly signIn: (
      client: string,
      email: Email,
      password: Redacted.Redacted<string>,
    ) => Effectful<
      SessionTokens,
      InvalidCredentials | EmailNotVerified | AccountLocked | StorageFailure
    >;
    readonly refresh: (token: string) => Effectful<SessionTokens, SessionExpired | StorageFailure>;
    readonly signOut: (token: string) => Effectful<void, StorageFailure>;
    readonly me: (viewer: Viewer["Service"]) => Effectful<Account, UserNotFound | StorageFailure>;
    readonly changePassword: (
      viewer: Viewer["Service"],
      current: Redacted.Redacted<string>,
      next: Redacted.Redacted<string>,
    ) => Effectful<SessionTokens, WrongPassword | UserNotFound | StorageFailure>;
    readonly list: Effectful<ReadonlyArray<Account>, StorageFailure>;
    readonly update: (
      viewer: Viewer["Service"],
      id: string,
      changes: UserChanges,
    ) => Effectful<Account, UserNotFound | OwnAccess | StorageFailure>;
    readonly grantLicence: (
      id: string,
      years: number,
    ) => Effectful<Account, UserNotFound | StorageFailure>;
    readonly purgeExpired: Effectful<void, StorageFailure>;
  }
>()("Accounts") {
  static readonly layer = Layer.effect(
    Accounts,
    Effect.gen(function* () {
      const sql = yield* SqlClient.SqlClient;
      const atomic = yield* Atomic;
      const passwords = yield* Passwords;
      const tokens = yield* Tokens;
      const throttle = yield* AuthThrottle;
      const mailer = yield* Mailer;
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
      const verifiedUsers = SqlSchema.findAll({
        Request: Schema.Void,
        Result: User,
        execute: () => sql`SELECT * FROM users WHERE email_verified_at IS NOT NULL ORDER BY email`,
      });
      const failuresFor = SqlSchema.findOneOption({
        Request: Schema.String,
        Result: FailureRow,
        execute: (email) =>
          sql`SELECT failures, locked_until FROM sign_in_failures WHERE email = ${email}`,
      });
      const codeFor = SqlSchema.findOneOption({
        Request: CodeKey,
        Result: CodeRow,
        execute: ({ email, purpose }) =>
          sql`SELECT code_hash, expires_at FROM email_codes WHERE email = ${email} AND purpose = ${purpose}`,
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

      const accountOf = Effect.fnUntraced(function* (user: User) {
        return toAccount(user, yield* DateTime.now);
      });

      const hashSecret = (secret: string) =>
        digest.digest(Digest.sha256, encoder.encode(secret)).pipe(Effect.map(Base64Url.encode));

      const hashCode = (email: Email, purpose: CodePurpose, code: string) =>
        digest.digestHex(Digest.sha256, encoder.encode(`${purpose}:${email}:${code}`));

      const newSecret = random.randomBytes(SECRET_BYTES).pipe(Effect.map(Base64Url.encode));

      const issue = Effect.fnUntraced(function* (user: User, sessionId: SessionId, secret: string) {
        const account = yield* accountOf(user);
        const accessToken = yield* tokens.issue({
          userId: user.id,
          sessionId,
          role: user.role,
          licence: account.licence,
        });
        return {
          accessToken,
          refreshToken: `${sessionId}.${secret}`,
          expiresIn: ACCESS_SECONDS,
          account,
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

      const passwordFields = Effect.fnUntraced(function* (password: Redacted.Redacted<string>) {
        const stored = yield* passwords.hash(password);
        return {
          password_hash: stored.hash,
          password_salt: stored.salt,
          password_iterations: stored.iterations,
          updated_at: millis(yield* DateTime.now),
        };
      });

      const replacePassword = Effect.fnUntraced(function* (
        user: User,
        password: Redacted.Redacted<string>,
      ) {
        const fields = yield* passwordFields(password);
        const { session, secret } = yield* newSession(user);
        const row = yield* encodeSession(session).pipe(Effect.orDie);
        yield* atomic
          .run([
            sql`UPDATE users SET ${sql.update(fields)} WHERE id = ${user.id}`,
            sql`DELETE FROM sessions WHERE user_id = ${user.id}`,
            sql`DELETE FROM sign_in_failures WHERE email = ${user.email}`,
            sql`INSERT INTO sessions ${sql.insert(row)}`,
          ])
          .pipe(storage("change the password"));
        return yield* issue(yield* existing(user.id), session.id, secret);
      });

      const sendMail = (message: MailMessage) =>
        mailer.send(message).pipe(storage("send the email"));

      const claimSend = Effect.fnUntraced(function* (
        email: Email,
        purpose: CodePurpose | "notice",
        codeHash: string,
        lifetime: Duration.Duration,
      ) {
        const now = millis(yield* DateTime.now);
        const claimed = yield* sql`INSERT INTO email_codes ${sql.insert({
          email,
          purpose,
          codeHash,
          expiresAt: now + Duration.toMillis(lifetime),
          attempts: 0,
          sentAt: now,
        })}
          ON CONFLICT (email, purpose) DO UPDATE SET
            code_hash = excluded.code_hash,
            expires_at = excluded.expires_at,
            attempts = 0,
            sent_at = excluded.sent_at
          WHERE email_codes.sent_at <= ${now - Duration.toMillis(CODE_COOLDOWN)}
          RETURNING email`.pipe(storage("store the code"));
        return claimed.length > 0;
      });

      const sendNotice = Effect.fnUntraced(function* (email: Email) {
        if (!(yield* claimSend(email, "notice", "", CODE_COOLDOWN))) return;
        yield* sendMail(accountExistsMessage(email));
      });

      const sendCode = Effect.fnUntraced(function* (email: Email, purpose: CodePurpose) {
        const code = String(yield* random.intBetween(0, 999_999)).padStart(6, "0");
        const codeHash = yield* hashCode(email, purpose, code);
        const claimed = yield* claimSend(email, purpose, codeHash, CODE_LIFETIME);
        if (!claimed) return;
        yield* sendMail(codeMessage(email, purpose, code)).pipe(
          Effect.tapError(() =>
            sql`DELETE FROM email_codes WHERE email = ${email} AND purpose = ${purpose} AND code_hash = ${codeHash}`.pipe(
              Effect.ignore,
            ),
          ),
        );
      });

      const redeem = Effect.fnUntraced(function* (
        email: Email,
        purpose: CodePurpose,
        code: string,
      ) {
        const now = millis(yield* DateTime.now);
        const row = yield* codeFor({ email, purpose }).pipe(storage("check the code"));
        if (Option.isNone(row)) return yield* wrongCode();
        const { codeHash, expiresAt } = row.value;
        const key = sql`email = ${email} AND purpose = ${purpose} AND code_hash = ${codeHash}`;
        if (expiresAt <= now) {
          yield* sql`DELETE FROM email_codes WHERE ${key}`.pipe(storage("check the code"));
          return yield* wrongCode();
        }
        if (sameText(yield* hashCode(email, purpose, code), codeHash)) {
          const used = yield* sql`DELETE FROM email_codes WHERE ${key} RETURNING email`.pipe(
            storage("use the code"),
          );
          if (used.length === 0) return yield* wrongCode();
          return;
        }
        yield* atomic
          .run([
            sql`UPDATE email_codes SET attempts = attempts + 1 WHERE ${key}`,
            sql`DELETE FROM email_codes WHERE ${key} AND attempts >= ${MAX_CODE_ATTEMPTS}`,
          ])
          .pipe(storage("check the code"));
        return yield* wrongCode();
      });

      const recordFailure = (email: string, now: number) =>
        sql`INSERT INTO sign_in_failures (email, failures, locked_until) VALUES (${email}, 1, 0)
            ON CONFLICT (email) DO UPDATE SET
              failures = CASE WHEN failures + 1 >= ${MAX_FAILURES} THEN 0 ELSE failures + 1 END,
              locked_until = CASE WHEN failures + 1 >= ${MAX_FAILURES} THEN ${now + Duration.toMillis(LOCK_LIFETIME)} ELSE locked_until END`;

      const signUp = Effect.fn("Accounts.signUp")(function* (client: string, request: SignUp) {
        yield* throttle.check(client);
        const fields = yield* passwordFields(request.password);
        const found = yield* userByEmail(request.email).pipe(storage("sign up"));
        const now = yield* DateTime.now;
        if (Option.isSome(found) && isVerified(found.value)) {
          return yield* sendNotice(request.email);
        }
        if (Option.isSome(found)) {
          yield* sql`UPDATE users SET ${sql.update({ ...fields, name: request.name })}
            WHERE id = ${found.value.id} AND email_verified_at IS NULL`.pipe(storage("sign up"));
        } else {
          const row = yield* Schema.encodeEffect(User.insert)(
            User.insert.make({
              id: UserId.make(yield* random.uuidV4()),
              email: request.email,
              name: request.name,
              role: "member",
              passwordHash: fields.password_hash,
              passwordSalt: fields.password_salt,
              passwordIterations: fields.password_iterations,
              disabled: false,
              emailVerifiedAt: Option.none(),
              trialEndsAt: Option.none(),
              paidUntil: Option.none(),
              createdAt: Model.Override(now),
              updatedAt: Model.Override(now),
            }),
          ).pipe(Effect.orDie);
          yield* sql`INSERT INTO users ${sql.insert(row)} ON CONFLICT (email) DO NOTHING`.pipe(
            storage("sign up"),
          );
        }
        yield* sendCode(request.email, "verify");
      });

      const requestCode = Effect.fn("Accounts.requestCode")(function* (
        client: string,
        email: Email,
        purpose: CodePurpose,
      ) {
        yield* throttle.check(client);
        const found = yield* userByEmail(email).pipe(storage("send a code"));
        if (Option.isNone(found)) return;
        const user = found.value;
        const eligible =
          purpose === "verify" ? !isVerified(user) : isVerified(user) && !user.disabled;
        if (eligible) yield* sendCode(email, purpose);
      });

      const verifyEmail = Effect.fn("Accounts.verifyEmail")(function* (
        client: string,
        email: Email,
        code: string,
      ) {
        yield* throttle.check(client);
        yield* redeem(email, "verify", code);
        const found = yield* userByEmail(email).pipe(storage("confirm the email"));
        if (Option.isNone(found) || found.value.disabled) return yield* wrongCode();
        const now = yield* DateTime.now;
        yield* sql`UPDATE users SET
            email_verified_at = COALESCE(email_verified_at, ${millis(now)}),
            trial_ends_at = COALESCE(trial_ends_at, ${millis(DateTime.add(now, TRIAL_LENGTH))}),
            updated_at = ${millis(now)}
          WHERE id = ${found.value.id}`.pipe(storage("confirm the email"));
        const user = yield* existing(found.value.id).pipe(
          Effect.catchTag("UserNotFound", () => Effect.fail(wrongCode())),
        );
        return yield* openSession(user).pipe(storage("open a session"));
      });

      const resetPassword = Effect.fn("Accounts.resetPassword")(function* (
        client: string,
        email: Email,
        code: string,
        password: Redacted.Redacted<string>,
      ) {
        yield* throttle.check(client);
        yield* redeem(email, "reset", code);
        const found = yield* userByEmail(email).pipe(storage("reset the password"));
        if (Option.isNone(found) || !isVerified(found.value) || found.value.disabled) {
          return yield* wrongCode();
        }
        return yield* replacePassword(found.value, password).pipe(
          Effect.catchTag("UserNotFound", () => Effect.fail(wrongCode())),
        );
      });

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
        const found = yield* userByEmail(email).pipe(storage("sign in"));
        const stored = Option.isSome(found) ? storedPassword(found.value) : yield* passwords.decoy;
        const matches = yield* passwords.verify(password, stored);
        if (Option.isNone(found) || !matches || found.value.disabled) {
          yield* recordFailure(email, now).pipe(storage("record a failed sign-in"));
          return yield* invalid();
        }
        yield* sql`DELETE FROM sign_in_failures WHERE email = ${email}`.pipe(storage("sign in"));
        if (!isVerified(found.value)) {
          yield* sendCode(email, "verify");
          return yield* new EmailNotVerified({ error: "confirm your email first" });
        }
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
        return yield* accountOf(yield* existing(viewer.id));
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
        return yield* replacePassword(user, next);
      });

      const list = Effect.gen(function* () {
        const now = yield* DateTime.now;
        const rows = yield* verifiedUsers(undefined).pipe(storage("list users"));
        return rows.map((user) => toAccount(user, now));
      }).pipe(Effect.withSpan("Accounts.list"));

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
        return yield* accountOf(yield* existing(user.id));
      });

      const grantLicence = Effect.fn("Accounts.grantLicence")(function* (
        id: string,
        years: number,
      ) {
        const user = yield* existing(id);
        const now = yield* DateTime.now;
        const from = Option.match(user.paidUntil, {
          onNone: () => now,
          onSome: (paid) => DateTime.max(paid, now),
        });
        yield* sql`UPDATE users SET ${sql.update({
          paidUntil: millis(DateTime.add(from, { years })),
          updatedAt: millis(now),
        })} WHERE id = ${user.id}`.pipe(storage("grant the licence"));
        return yield* accountOf(yield* existing(user.id));
      });

      const purgeExpired = Effect.gen(function* () {
        const now = millis(yield* DateTime.now);
        yield* atomic.run([
          sql`DELETE FROM sessions WHERE expires_at <= ${now}`,
          sql`DELETE FROM sign_in_failures WHERE locked_until <= ${now - Duration.toMillis(Duration.days(1))}`,
          sql`DELETE FROM email_codes WHERE expires_at <= ${now}`,
          sql`DELETE FROM users WHERE email_verified_at IS NULL AND created_at <= ${now - Duration.toMillis(UNVERIFIED_LIFETIME)}`,
        ]);
      }).pipe(storage("purge expired records"), Effect.withSpan("Accounts.purgeExpired"));

      return Accounts.of({
        signUp,
        requestCode,
        verifyEmail,
        resetPassword,
        signIn,
        refresh,
        signOut,
        me,
        changePassword,
        list,
        update,
        grantLicence,
        purgeExpired,
      });
    }),
  );
}

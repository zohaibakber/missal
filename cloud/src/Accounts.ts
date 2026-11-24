import * as Config from "effect/Config";
import * as Context from "effect/Context";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as Password from "effect/crypto/Password";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import type * as Redacted from "effect/Redacted";
import * as Model from "effect/schema/Model";
import { EmailNotVerified, InvalidCredentials, type SignUp, Viewer, WrongPassword } from "./Api.ts";
import { Codes, wrongCode } from "./Codes.ts";
import { Atomic } from "./Database.ts";
import { storage } from "./Failures.ts";
import { type CodePurpose, type Email, TRIAL_LENGTH, User, UserId } from "./Models.ts";
import { DECOY } from "./Passwords.ts";
import { Sessions } from "./Sessions.ts";
import { SignInLockout } from "./Throttle.ts";
import { isVerified, toAccount, Users } from "./Users.ts";

const UNVERIFIED_LIFETIME = { days: 7 } as const;

export const VERIFY_EMAILS_VARIABLE = "VERIFY_EMAILS";

const emailVerification = Config.Boolean(VERIFY_EMAILS_VARIABLE).pipe(
  Config.withDefault(false),
  Effect.orDie,
);

const invalid = () => new InvalidCredentials({ error: "wrong email or password" });

export class Accounts extends Context.Service<Accounts>()("Accounts", {
  make: Effect.gen(function* () {
    const atomic = yield* Atomic;
    const passwords = yield* Password.Password;
    const random = yield* SecureRandom.SecureRandom;
    const lockout = yield* SignInLockout;
    const users = yield* Users;
    const sessions = yield* Sessions;
    const codes = yield* Codes;

    const hash = (password: Redacted.Redacted<string>) =>
      passwords.hash(password).pipe(Effect.orDie);

    const matches = (password: Redacted.Redacted<string>, stored: string) =>
      passwords.verify(password, stored).pipe(Effect.orDie);

    const replacePassword = Effect.fnUntraced(function* (
      user: User,
      password: Redacted.Redacted<string>,
    ) {
      const stored = yield* hash(password);
      const now = yield* DateTime.now;
      const session = yield* sessions.prepare(user);
      yield* atomic
        .run([
          users.update(user.id, { password: stored }, now),
          sessions.endAll(user.id),
          lockout.clear(user.email),
          session.insert,
        ])
        .pipe(storage("change the password"));
      return yield* sessions.issue(yield* users.existing(user.id), session.id, session.secret);
    });

    const signUp = Effect.fn("Accounts.signUp")(function* (request: SignUp) {
      const verifyEmails = yield* emailVerification;
      const password = yield* hash(request.password);
      const found = yield* users.byEmail(request.email);
      const now = yield* DateTime.now;
      if (Option.isSome(found) && isVerified(found.value)) {
        if (verifyEmails) yield* codes.notice(request.email);
        return;
      }
      if (Option.isSome(found)) {
        yield* users.replaceUnverified(found.value.id, request.name, password, now);
      } else {
        yield* users.insert(
          User.insert.make({
            id: UserId.make(yield* random.uuidV4()),
            email: request.email,
            name: request.name,
            role: "member",
            password,
            disabled: false,
            emailVerifiedAt: verifyEmails ? Option.none() : Option.some(now),
            trialEndsAt: verifyEmails
              ? Option.none()
              : Option.some(DateTime.add(now, TRIAL_LENGTH)),
            paidUntil: Option.none(),
            createdAt: Model.Override(now),
            updatedAt: Model.Override(now),
          }),
        );
      }
      if (verifyEmails) yield* codes.send(request.email, "verify");
    });

    const requestCode = Effect.fn("Accounts.requestCode")(function* (
      email: Email,
      purpose: CodePurpose,
    ) {
      const found = yield* users.byEmail(email);
      if (Option.isNone(found)) return;
      const user = found.value;
      const eligible =
        purpose === "verify" ? !isVerified(user) : isVerified(user) && !user.disabled;
      if (eligible) yield* codes.send(email, purpose);
    });

    const verifyEmail = Effect.fn("Accounts.verifyEmail")(function* (email: Email, code: string) {
      yield* codes.redeem(email, "verify", code);
      const found = yield* users.byEmail(email);
      if (Option.isNone(found) || found.value.disabled) return yield* wrongCode();
      yield* users.confirm(found.value.id, yield* DateTime.now).pipe(storage("confirm the email"));
      const user = yield* users
        .existing(found.value.id)
        .pipe(Effect.catchTag("UserNotFound", () => Effect.fail(wrongCode())));
      return yield* sessions.open(user);
    });

    const resetPassword = Effect.fn("Accounts.resetPassword")(function* (
      email: Email,
      code: string,
      password: Redacted.Redacted<string>,
    ) {
      yield* codes.redeem(email, "reset", code);
      const found = yield* users.byEmail(email);
      if (Option.isNone(found) || !isVerified(found.value) || found.value.disabled) {
        return yield* wrongCode();
      }
      return yield* replacePassword(found.value, password).pipe(
        Effect.catchTag("UserNotFound", () => Effect.fail(wrongCode())),
      );
    });

    const signIn = Effect.fn("Accounts.signIn")(function* (
      email: Email,
      password: Redacted.Redacted<string>,
    ) {
      const now = yield* DateTime.now;
      yield* lockout.check(email, now);
      const found = yield* users.byEmail(email);
      const stored = Option.isSome(found) ? found.value.password : DECOY;
      const correct = yield* matches(password, stored);
      if (Option.isNone(found) || !correct || found.value.disabled) {
        yield* lockout.recordFailure(email, now);
        return yield* invalid();
      }
      const user = found.value;
      yield* lockout.clear(email).pipe(storage("sign in"));
      if (!isVerified(user)) {
        yield* codes.send(email, "verify");
        return yield* new EmailNotVerified({ error: "confirm your email first" });
      }
      if (yield* passwords.needsRehash(user.password).pipe(Effect.orDie)) {
        yield* users
          .update(user.id, { password: yield* hash(password) }, now)
          .pipe(storage("sign in"));
      }
      return yield* sessions.open(user);
    });

    const me = Effect.gen(function* () {
      const viewer = yield* Viewer;
      return toAccount(yield* users.existing(viewer.id), yield* DateTime.now);
    }).pipe(Effect.withSpan("Accounts.me"));

    const changePassword = Effect.fn("Accounts.changePassword")(function* (
      current: Redacted.Redacted<string>,
      next: Redacted.Redacted<string>,
    ) {
      const viewer = yield* Viewer;
      const user = yield* users.existing(viewer.id);
      if (!(yield* matches(current, user.password))) {
        return yield* new WrongPassword({ error: "the current password is wrong" });
      }
      return yield* replacePassword(user, next);
    });

    const purgeExpired = Effect.gen(function* () {
      const now = yield* DateTime.now;
      yield* atomic.run([
        sessions.purge(now),
        lockout.purge(now),
        codes.purge(now),
        users.purgeUnverified(DateTime.subtract(now, UNVERIFIED_LIFETIME)),
      ]);
    }).pipe(storage("purge expired records"), Effect.withSpan("Accounts.purgeExpired"));

    return {
      signUp,
      requestCode,
      verifyEmail,
      resetPassword,
      signIn,
      me,
      changePassword,
      purgeExpired,
    };
  }),
}) {
  static readonly layer = Layer.effect(Accounts, Accounts.make);
}

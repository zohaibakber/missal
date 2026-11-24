import * as Context from "effect/Context";
import * as Password from "effect/crypto/Password";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import type * as Redacted from "effect/Redacted";
import * as Model from "effect/schema/Model";
import {
  EmailTaken,
  type ManagedAccount,
  type NewUser,
  OwnAccess,
  type UserChanges,
  type Viewer,
} from "./Api.ts";
import { Codes } from "./Codes.ts";
import { Atomic } from "./Database.ts";
import { storage } from "./Failures.ts";
import { TRIAL_LENGTH, User, UserId } from "./Models.ts";
import { type Activity, Sessions } from "./Sessions.ts";
import { SignInLockout } from "./Throttle.ts";
import { isVerified, toAccount, Users } from "./Users.ts";

const toManaged = (user: User, now: DateTime.Utc, activity?: Activity): ManagedAccount => ({
  ...toAccount(user, now),
  sessions: activity?.sessions ?? 0,
  lastSeenAt: activity?.lastSeenAt ?? null,
});

const later = (end: Option.Option<DateTime.Utc>, now: DateTime.Utc) =>
  Option.match(end, { onNone: () => now, onSome: (until) => DateTime.max(until, now) });

const ownAccess = (error: string) => new OwnAccess({ error });

export class People extends Context.Service<People>()("People", {
  make: Effect.gen(function* () {
    const atomic = yield* Atomic;
    const passwords = yield* Password.Password;
    const random = yield* SecureRandom.SecureRandom;
    const lockout = yield* SignInLockout;
    const users = yield* Users;
    const sessions = yield* Sessions;
    const codes = yield* Codes;

    const managed = Effect.fnUntraced(function* (id: string) {
      const user = yield* users.existing(id);
      const now = yield* DateTime.now;
      const [activity] = yield* sessions.activity(now, user.id);
      return toManaged(user, now, activity);
    });

    const list = Effect.gen(function* () {
      const now = yield* DateTime.now;
      const all = yield* users.all;
      const seen = yield* sessions.activity(now, null);
      const byUser = new Map(seen.map((row) => [row.userId, row]));
      return all.map((user) => toManaged(user, now, byUser.get(user.id)));
    }).pipe(Effect.withSpan("People.list"));

    const create = Effect.fn("People.create")(function* (request: NewUser) {
      const password = yield* passwords.hash(request.password).pipe(Effect.orDie);
      const now = yield* DateTime.now;
      const id = UserId.make(yield* random.uuidV4());
      const inserted = yield* users.insert(
        User.insert.make({
          id,
          email: request.email,
          name: request.name,
          role: request.role,
          password,
          disabled: false,
          emailVerifiedAt: Option.some(now),
          trialEndsAt: Option.some(DateTime.add(now, TRIAL_LENGTH)),
          paidUntil:
            request.months > 0
              ? Option.some(DateTime.add(now, { months: request.months }))
              : Option.none(),
          createdAt: Model.Override(now),
          updatedAt: Model.Override(now),
        }),
      );
      if (!inserted) {
        return yield* new EmailTaken({ error: "an account with this email already exists" });
      }
      yield* codes.discard(request.email).pipe(storage("create the account"));
      return yield* managed(id).pipe(Effect.catchTag("UserNotFound", Effect.die));
    });

    const update = Effect.fn("People.update")(function* (
      viewer: Viewer["Service"],
      id: string,
      changes: UserChanges,
    ) {
      const demotesSelf =
        id === viewer.id &&
        ((changes.role !== undefined && changes.role !== "admin") || changes.disabled === true);
      if (demotesSelf) {
        return yield* ownAccess("you cannot remove your own administrator access");
      }
      const user = yield* users.existing(id);
      yield* atomic
        .run([
          users.update(user.id, changes, yield* DateTime.now),
          ...(changes.disabled === true ? [sessions.endAll(user.id)] : []),
        ])
        .pipe(storage("update the user"));
      return yield* managed(user.id);
    });

    const remove = Effect.fn("People.remove")(function* (viewer: Viewer["Service"], id: string) {
      if (id === viewer.id) return yield* ownAccess("you cannot delete your own account");
      const user = yield* users.existing(id);
      yield* atomic
        .run([
          sessions.endAll(user.id),
          codes.discard(user.email),
          lockout.clear(user.email),
          users.remove(user.id),
        ])
        .pipe(storage("delete the account"));
    });

    const confirmEmail = Effect.fn("People.confirmEmail")(function* (id: string) {
      const user = yield* users.existing(id);
      yield* atomic
        .run([users.confirm(user.id, yield* DateTime.now), codes.discard(user.email, "verify")])
        .pipe(storage("confirm the email"));
      return yield* managed(user.id);
    });

    const grantLicence = Effect.fn("People.grantLicence")(function* (id: string, months: number) {
      const user = yield* users.existing(id);
      const now = yield* DateTime.now;
      const paidUntil = DateTime.add(later(user.paidUntil, now), { months });
      yield* users.update(user.id, { paidUntil }, now).pipe(storage("grant the licence"));
      return yield* managed(user.id);
    });

    const endLicence = Effect.fn("People.endLicence")(function* (id: string) {
      const user = yield* users.existing(id);
      yield* users.endLicence(user.id, yield* DateTime.now).pipe(storage("end the licence"));
      return yield* managed(user.id);
    });

    const extendTrial = Effect.fn("People.extendTrial")(function* (id: string, days: number) {
      const user = yield* users.existing(id);
      const now = yield* DateTime.now;
      const trialEndsAt = DateTime.add(later(user.trialEndsAt, now), { days });
      yield* users.update(user.id, { trialEndsAt }, now).pipe(storage("extend the trial"));
      return yield* managed(user.id);
    });

    const setPassword = Effect.fn("People.setPassword")(function* (
      viewer: Viewer["Service"],
      id: string,
      password: Redacted.Redacted<string>,
    ) {
      if (id === viewer.id) {
        return yield* ownAccess("change your own password from your account menu");
      }
      const user = yield* users.existing(id);
      const stored = yield* passwords.hash(password).pipe(Effect.orDie);
      yield* atomic
        .run([
          users.update(user.id, { password: stored }, yield* DateTime.now),
          sessions.endAll(user.id),
          lockout.clear(user.email),
        ])
        .pipe(storage("set the password"));
      return yield* managed(user.id);
    });

    const sendReset = Effect.fn("People.sendReset")(function* (id: string) {
      const user = yield* users.existing(id);
      if (isVerified(user) && !user.disabled) yield* codes.send(user.email, "reset");
    });

    const endSessions = Effect.fn("People.endSessions")(function* (
      viewer: Viewer["Service"],
      id: string,
    ) {
      if (id === viewer.id) return yield* ownAccess("sign out from your account menu");
      const user = yield* users.existing(id);
      yield* sessions.endAll(user.id).pipe(storage("sign out"));
      return yield* managed(user.id);
    });

    return {
      list,
      create,
      update,
      remove,
      confirmEmail,
      grantLicence,
      endLicence,
      extendTrial,
      setPassword,
      sendReset,
      endSessions,
    };
  }),
}) {
  static readonly layer = Layer.effect(People, People.make);
}

import { and, count, eq, gt, lte, max } from "drizzle-orm";
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
import * as Schema from "effect/Schema";
import { SessionExpired, type Session as SessionTokens } from "./Api.ts";
import { Db } from "./Database.ts";
import { storage } from "./Failures.ts";
import { Session, SessionId, type User } from "./Models.ts";
import { sessions } from "./Schema.ts";
import { ACCESS_SECONDS, Tokens } from "./Tokens.ts";
import { millis, toAccount, Users } from "./Users.ts";

const LIFETIME = Duration.days(60);
const SECRET_BYTES = 32;

export const Activity = Schema.Struct({
  userId: Schema.String,
  sessions: Schema.Int,
  lastSeenAt: Schema.NullOr(Schema.DateTimeUtcFromMillis),
});
export type Activity = typeof Activity.Type;

const encoder = new TextEncoder();

const decodeSession = Schema.decodeUnknownEffect(Session);
const decodeActivity = Schema.decodeUnknownEffect(Schema.Array(Activity));
const encodeInsert = Schema.encodeEffect(Session.insert);

const sameText = (left: string, right: string) =>
  Mac.timingSafeEqual(encoder.encode(left), encoder.encode(right));

const splitToken = (token: string) => {
  const dot = token.indexOf(".");
  return dot > 0 && dot < token.length - 1
    ? Option.some({ sessionId: SessionId.make(token.slice(0, dot)), secret: token.slice(dot + 1) })
    : Option.none();
};

const expired = () => new SessionExpired({ error: "your session has ended; sign in again" });

export class Sessions extends Context.Service<Sessions>()("Sessions", {
  make: Effect.gen(function* () {
    const db = yield* Db;
    const random = yield* SecureRandom.SecureRandom;
    const digest = yield* Digest.Digest;
    const tokens = yield* Tokens;
    const users = yield* Users;

    const findSession = (id: SessionId) =>
      db
        .select()
        .from(sessions)
        .where(eq(sessions.id, id))
        .pipe(
          Effect.flatMap((rows) =>
            rows[0] === undefined ? Effect.succeedNone : Effect.asSome(decodeSession(rows[0])),
          ),
          storage("refresh the session"),
        );

    const hashSecret = (secret: string) =>
      digest.digest(Digest.sha256, encoder.encode(secret)).pipe(Effect.map(Base64Url.encode));

    const newSecret = random.randomBytes(SECRET_BYTES).pipe(Effect.map(Base64Url.encode));

    const issue = Effect.fnUntraced(function* (user: User, sessionId: SessionId, secret: string) {
      const account = toAccount(user, yield* DateTime.now);
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

    const prepare = Effect.fnUntraced(function* (user: User) {
      const secret = yield* newSecret;
      const now = yield* DateTime.now;
      const session = Session.insert.make({
        id: SessionId.make(yield* random.uuidV4()),
        userId: user.id,
        refreshHash: yield* hashSecret(secret),
        expiresAt: DateTime.addDuration(now, LIFETIME),
      });
      const row = yield* encodeInsert(session).pipe(Effect.orDie);
      return { id: session.id, secret, insert: db.insert(sessions).values(row) };
    });

    const open = Effect.fnUntraced(function* (user: User) {
      const prepared = yield* prepare(user);
      yield* prepared.insert.pipe(storage("open a session"));
      return yield* issue(user, prepared.id, prepared.secret);
    });

    const refresh = Effect.fn("Sessions.refresh")(function* (token: string) {
      const parts = splitToken(token);
      if (Option.isNone(parts)) return yield* expired();
      const { sessionId, secret } = parts.value;
      const now = yield* DateTime.now;
      const session = yield* findSession(sessionId);
      if (Option.isNone(session)) return yield* expired();
      const user = yield* users.find(session.value.userId);
      const presented = yield* hashSecret(secret);
      const valid =
        Option.isSome(user) &&
        !user.value.disabled &&
        DateTime.isGreaterThan(session.value.expiresAt, now) &&
        sameText(presented, session.value.refreshHash);
      if (!valid || Option.isNone(user)) {
        yield* db
          .delete(sessions)
          .where(eq(sessions.id, sessionId))
          .pipe(storage("end the session"));
        return yield* expired();
      }
      const next = yield* newSecret;
      const rotated = yield* db
        .update(sessions)
        .set({
          refreshHash: yield* hashSecret(next),
          lastUsedAt: millis(now),
          expiresAt: millis(DateTime.addDuration(now, LIFETIME)),
        })
        .where(and(eq(sessions.id, sessionId), eq(sessions.refreshHash, session.value.refreshHash)))
        .returning({ id: sessions.id })
        .pipe(storage("refresh the session"));
      if (rotated.length === 0) return yield* expired();
      return yield* issue(user.value, sessionId, next);
    });

    const revoke = Effect.fn("Sessions.revoke")(function* (token: string) {
      const parts = splitToken(token);
      if (Option.isNone(parts)) return;
      const presented = yield* hashSecret(parts.value.secret);
      yield* db
        .delete(sessions)
        .where(and(eq(sessions.id, parts.value.sessionId), eq(sessions.refreshHash, presented)))
        .pipe(storage("sign out"));
    });

    const endAll = (userId: string) => db.delete(sessions).where(eq(sessions.userId, userId));

    const activity = (now: DateTime.Utc, userId: string | null) =>
      db
        .select({
          userId: sessions.userId,
          sessions: count(),
          lastSeenAt: max(sessions.lastUsedAt),
        })
        .from(sessions)
        .where(
          and(
            gt(sessions.expiresAt, millis(now)),
            userId === null ? undefined : eq(sessions.userId, userId),
          ),
        )
        .groupBy(sessions.userId)
        .pipe(Effect.flatMap(decodeActivity), storage("read the sessions"));

    const purge = (now: DateTime.Utc) =>
      db.delete(sessions).where(lte(sessions.expiresAt, millis(now)));

    return { issue, prepare, open, refresh, revoke, endAll, activity, purge };
  }),
}) {
  static readonly layer = Layer.effect(Sessions, Sessions.make);
}

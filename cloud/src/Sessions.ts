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
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlModel from "effect/sql/SqlModel";
import * as SqlSchema from "effect/sql/SqlSchema";
import { SessionExpired, type Session as SessionTokens } from "./Api.ts";
import { storage } from "./Failures.ts";
import { Session, SessionId, type User } from "./Models.ts";
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
    const sql = yield* SqlClient.SqlClient;
    const random = yield* SecureRandom.SecureRandom;
    const digest = yield* Digest.Digest;
    const tokens = yield* Tokens;
    const users = yield* Users;
    const repository = yield* SqlModel.makeRepository(Session, {
      tableName: "sessions",
      spanPrefix: "Sessions",
      idColumn: "id",
    });
    const encodeInsert = Schema.encodeEffect(Session.insert);

    const findActivity = SqlSchema.findAll({
      Request: Schema.Struct({ now: Schema.Number, id: Schema.NullOr(Schema.String) }),
      Result: Activity,
      execute: ({ now, id }) =>
        sql`SELECT user_id, COUNT(*) AS sessions, MAX(last_used_at) AS last_seen_at
          FROM sessions
          WHERE expires_at > ${now} ${id === null ? sql`` : sql`AND user_id = ${id}`}
          GROUP BY user_id`,
    });

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
      return { id: session.id, secret, insert: sql`INSERT INTO sessions ${sql.insert(row)}` };
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
      const session = yield* repository
        .findById(sessionId)
        .pipe(Effect.catchNoSuchElement, storage("refresh the session"));
      if (Option.isNone(session)) return yield* expired();
      const user = yield* users.find(session.value.userId);
      const presented = yield* hashSecret(secret);
      const valid =
        Option.isSome(user) &&
        !user.value.disabled &&
        DateTime.isGreaterThan(session.value.expiresAt, now) &&
        sameText(presented, session.value.refreshHash);
      if (!valid || Option.isNone(user)) {
        yield* repository.delete(sessionId).pipe(storage("end the session"));
        return yield* expired();
      }
      const next = yield* newSecret;
      const rotated = yield* sql`UPDATE sessions SET ${sql.update({
        refreshHash: yield* hashSecret(next),
        lastUsedAt: millis(now),
        expiresAt: millis(DateTime.addDuration(now, LIFETIME)),
      })} WHERE id = ${sessionId} AND refresh_hash = ${session.value.refreshHash} RETURNING id`.pipe(
        storage("refresh the session"),
      );
      if (rotated.length === 0) return yield* expired();
      return yield* issue(user.value, sessionId, next);
    });

    const revoke = Effect.fn("Sessions.revoke")(function* (token: string) {
      const parts = splitToken(token);
      if (Option.isNone(parts)) return;
      const presented = yield* hashSecret(parts.value.secret);
      yield* sql`DELETE FROM sessions WHERE id = ${parts.value.sessionId} AND refresh_hash = ${presented}`.pipe(
        storage("sign out"),
      );
    });

    const endAll = (userId: string) => sql`DELETE FROM sessions WHERE user_id = ${userId}`;

    const activity = (now: DateTime.Utc, userId: string | null) =>
      findActivity({ now: millis(now), id: userId }).pipe(storage("read the sessions"));

    const purge = (now: DateTime.Utc) =>
      sql`DELETE FROM sessions WHERE expires_at <= ${millis(now)}`;

    return { issue, prepare, open, refresh, revoke, endAll, activity, purge };
  }),
}) {
  static readonly layer = Layer.effect(Sessions, Sessions.make);
}

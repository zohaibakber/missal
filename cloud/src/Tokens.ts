import * as Context from "effect/Context";
import * as Mac from "effect/crypto/Mac";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import type * as Jwk from "effect/jose/Jwk";
import * as Jwt from "effect/jose/Jwt";
import * as Layer from "effect/Layer";
import * as Redacted from "effect/Redacted";
import * as Schema from "effect/Schema";
import { Unauthorized, Viewer } from "./Api.ts";
import { type Licence, Role, SessionId, UserId } from "./Models.ts";

export const ACCESS_SECONDS = 15 * 60;
const ISSUER = "missal";
const TYPE = "at+jwt";

const Claims = Schema.Struct({
  iss: Schema.Literal(ISSUER),
  sub: UserId,
  sid: SessionId,
  iat: Schema.Int,
  exp: Schema.Int,
  role: Role,
  lic: Schema.Int,
});

export interface Subject {
  readonly userId: UserId;
  readonly sessionId: SessionId;
  readonly role: Role;
  readonly licence: Licence;
}

const licensedUntil = (licence: Licence) =>
  licence.status === "expired" || licence.status === "trialEnded"
    ? 0
    : DateTime.toEpochSeconds(licence.until);

const unauthorized = () => new Unauthorized({ error: "sign in to use Missal" });

export class SigningKey extends Context.Service<SigningKey, Effect.Effect<CryptoKey>>()(
  "SigningKey",
) {}

const isSecret = (key: CryptoKey): key is CryptoKey & { readonly type: "secret" } =>
  key.type === "secret";

const hs256 = (cryptoKey: CryptoKey) =>
  (isSecret(cryptoKey)
    ? Mac.fromCryptoKey(Mac.hmacSha256, cryptoKey)
    : Effect.die("the token signing key must be a secret HMAC key")
  ).pipe(
    Effect.map((key): Jwk.SecretKey<"HS256"> => ({
      _tag: "SecretKey",
      algorithm: "HS256",
      kid: undefined,
      key,
    })),
    Effect.orDie,
  );

export class Tokens extends Context.Service<Tokens>()("Tokens", {
  make: Effect.gen(function* () {
    const mac = yield* Mac.Mac;
    const signingKey = yield* Effect.cached(Effect.flatMap(yield* SigningKey, hs256));

    const issue = Effect.fn("Tokens.issue")(function* (subject: Subject) {
      const key = yield* signingKey;
      const now = DateTime.toEpochSeconds(yield* DateTime.now);
      const token = yield* Jwt.sign(
        Claims,
        {
          iss: ISSUER,
          sub: subject.userId,
          sid: subject.sessionId,
          iat: now,
          exp: now + ACCESS_SECONDS,
          role: subject.role,
          lic: licensedUntil(subject.licence),
        },
        key,
        { typ: TYPE },
      ).pipe(Effect.orDie, Effect.provideService(Mac.Mac, mac));
      return Redacted.value(token);
    });

    const verify = Effect.fn("Tokens.verify")(function* (token: Redacted.Redacted<string>) {
      const key = yield* signingKey;
      const { claims } = yield* Jwt.verify(Claims, token, key, {
        issuer: ISSUER,
        typ: TYPE,
        requiredClaims: ["sub", "exp", "iat"],
      }).pipe(Effect.mapError(unauthorized), Effect.provideService(Mac.Mac, mac));
      return Viewer.of({
        id: claims.sub,
        sessionId: claims.sid,
        role: claims.role,
        licensedUntil: DateTime.fromEpochSeconds(claims.lic),
      });
    });

    return { issue, verify };
  }),
}) {
  static readonly layer = Layer.effect(Tokens, Tokens.make);
}

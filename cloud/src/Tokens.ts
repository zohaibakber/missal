import * as Config from "effect/Config";
import * as Context from "effect/Context";
import * as Mac from "effect/crypto/Mac";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as Base64Url from "effect/encoding/Base64Url";
import * as Hex from "effect/encoding/Hex";
import * as Jwk from "effect/jose/Jwk";
import * as Jwt from "effect/jose/Jwt";
import * as Layer from "effect/Layer";
import * as Redacted from "effect/Redacted";
import * as Result from "effect/Result";
import * as Schema from "effect/Schema";
import { Unauthorized, Viewer } from "./Api.ts";
import { type Licence, Role, SessionId, UserId } from "./Models.ts";

export const ACCESS_SECONDS = 15 * 60;
export const SECRET_VARIABLE = "TOKEN_SECRET";
const ISSUER = "missal";
const TYPE = "at+jwt";
const MIN_SECRET_BYTES = 32;

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

const seconds = (time: DateTime.Utc) => Math.floor(DateTime.toEpochMillis(time) / 1000);

const licensedUntil = (licence: Licence) =>
  licence.status === "expired" || licence.status === "trialEnded" ? 0 : seconds(licence.until);

const unauthorized = () => new Unauthorized({ error: "sign in to use Missal" });

const secretBytes = (secret: Redacted.Redacted<string>) => {
  const bytes = Hex.decode(Redacted.value(secret).trim());
  return Result.isSuccess(bytes) && bytes.success.length >= MIN_SECRET_BYTES
    ? Effect.succeed(bytes.success)
    : Effect.die(`${SECRET_VARIABLE} must be at least ${MIN_SECRET_BYTES} hex-encoded bytes`);
};

export class Tokens extends Context.Service<
  Tokens,
  {
    readonly issue: (subject: Subject) => Effect.Effect<string>;
    readonly verify: (
      token: Redacted.Redacted<string>,
    ) => Effect.Effect<Viewer["Service"], Unauthorized>;
  }
>()("Tokens") {
  static readonly layer = Layer.effect(
    Tokens,
    Effect.gen(function* () {
      const mac = yield* Mac.Mac;
      const secret = yield* Config.Redacted(SECRET_VARIABLE).pipe(Effect.orDie);
      const bytes = yield* secretBytes(secret);
      const key = yield* Jwk.importSecret(
        Redacted.make({ kty: "oct", k: Base64Url.encode(bytes) } as const),
        "HS256",
      ).pipe(Effect.orDie);

      const issue = Effect.fn("Tokens.issue")(function* (subject: Subject) {
        const now = seconds(yield* DateTime.now);
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
        const { claims } = yield* Jwt.verify(Claims, token, key, {
          issuer: ISSUER,
          typ: TYPE,
          requiredClaims: ["sub", "exp", "iat"],
        }).pipe(Effect.mapError(unauthorized), Effect.provideService(Mac.Mac, mac));
        return Viewer.of({
          id: claims.sub,
          sessionId: claims.sid,
          role: claims.role,
          licensedUntil: DateTime.makeUnsafe(claims.lic * 1000),
        });
      });

      return Tokens.of({ issue, verify });
    }),
  );
}

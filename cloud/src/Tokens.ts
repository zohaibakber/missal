import * as Clock from "effect/Clock";
import * as Context from "effect/Context";
import * as Mac from "effect/crypto/Mac";
import * as Effect from "effect/Effect";
import * as Jwk from "effect/jose/Jwk";
import * as Jwt from "effect/jose/Jwt";
import * as Layer from "effect/Layer";
import * as Redacted from "effect/Redacted";
import * as Schema from "effect/Schema";
import { Role, Unauthorized, Viewer } from "./Api.ts";

export const ACCESS_SECONDS = 15 * 60;
const ISSUER = "missal";
const TYPE = "at+jwt";

const Claims = Schema.Struct({
  iss: Schema.Literal(ISSUER),
  sub: Schema.String,
  sid: Schema.String,
  iat: Schema.Int,
  exp: Schema.Int,
  role: Role,
  mcp: Schema.Boolean,
});

export interface Subject {
  readonly userId: string;
  readonly sessionId: string;
  readonly role: Role;
  readonly mustChangePassword: boolean;
}

const unauthorized = () => new Unauthorized({ error: "sign in to use Missal" });

export class Tokens extends Context.Service<
  Tokens,
  {
    readonly issue: (subject: Subject) => Effect.Effect<string>;
    readonly verify: (
      token: Redacted.Redacted<string>,
    ) => Effect.Effect<Viewer["Service"], Unauthorized>;
  }
>()("missal/Tokens") {
  static readonly layer = (secret: Redacted.Redacted<string>) =>
    Layer.effect(
      Tokens,
      Effect.gen(function* () {
        const mac = yield* Mac.Mac;
        const key = yield* Jwk.importSecret(
          Redacted.make({ kty: "oct", k: Redacted.value(secret) } as const),
          "HS256",
        ).pipe(Effect.orDie);

        const issue = Effect.fn("Tokens.issue")(function* (subject: Subject) {
          const now = Math.floor((yield* Clock.currentTimeMillis) / 1000);
          const token = yield* Jwt.sign(
            Claims,
            {
              iss: ISSUER,
              sub: subject.userId,
              sid: subject.sessionId,
              iat: now,
              exp: now + ACCESS_SECONDS,
              role: subject.role,
              mcp: subject.mustChangePassword,
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
            mustChangePassword: claims.mcp,
          });
        });

        return Tokens.of({ issue, verify });
      }),
    );
}

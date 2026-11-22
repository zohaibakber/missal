import * as Context from "effect/Context";
import * as Digest from "effect/crypto/Digest";
import * as Kdf from "effect/crypto/Kdf";
import * as Password from "effect/crypto/Password";
import * as Effect from "effect/Effect";
import * as Base64Url from "effect/encoding/Base64Url";
import * as Layer from "effect/Layer";
import type * as Redacted from "effect/Redacted";

export const ITERATIONS = 100_000;

export interface StoredPassword {
  readonly hash: string;
  readonly salt: string;
  readonly iterations: number;
}

export const DECOY: StoredPassword = {
  hash: Base64Url.encode(new Uint8Array(32)),
  salt: Base64Url.encode(new Uint8Array(16)),
  iterations: ITERATIONS,
};

const pbkdf2 = (iterations: number) => Kdf.pbkdf2({ hash: Digest.sha256, iterations });

export class Passwords extends Context.Service<
  Passwords,
  {
    readonly hash: (password: Redacted.Redacted<string>) => Effect.Effect<StoredPassword>;
    readonly verify: (
      password: Redacted.Redacted<string>,
      stored: StoredPassword,
    ) => Effect.Effect<boolean>;
  }
>()("Passwords") {
  static readonly layer = Layer.effect(
    Passwords,
    Effect.gen(function* () {
      const passwords = yield* Password.Password;

      const hash = Effect.fn("Passwords.hash")(function* (password: Redacted.Redacted<string>) {
        const decoded = yield* passwords
          .hash(password)
          .pipe(Effect.flatMap(Password.decode), Effect.orDie);
        return {
          hash: Base64Url.encode(decoded.hash),
          salt: Base64Url.encode(decoded.salt),
          iterations: ITERATIONS,
        };
      });

      const verify = Effect.fn("Passwords.verify")(function* (
        password: Redacted.Redacted<string>,
        stored: StoredPassword,
      ) {
        const salt = yield* Effect.fromResult(Base64Url.decode(stored.salt));
        const hash = yield* Effect.fromResult(Base64Url.decode(stored.hash));
        const phc = Password.encode({ algorithm: pbkdf2(stored.iterations), salt, hash });
        return yield* passwords.verify(password, phc);
      }, Effect.orDie);

      return Passwords.of({ hash, verify });
    }),
  ).pipe(Layer.provide(Password.layer({ algorithm: pbkdf2(ITERATIONS) })));
}

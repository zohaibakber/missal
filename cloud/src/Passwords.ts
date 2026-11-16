import * as Context from "effect/Context";
import * as Digest from "effect/crypto/Digest";
import * as Kdf from "effect/crypto/Kdf";
import * as Mac from "effect/crypto/Mac";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as Effect from "effect/Effect";
import * as Base64Url from "effect/encoding/Base64Url";
import * as Layer from "effect/Layer";
import * as Redacted from "effect/Redacted";
import * as Result from "effect/Result";

export const ITERATIONS = 100_000;
const SALT_BYTES = 16;
const HASH_BYTES = 32;

export interface StoredPassword {
  readonly hash: string;
  readonly salt: string;
  readonly iterations: number;
}

const encoder = new TextEncoder();

export class Passwords extends Context.Service<
  Passwords,
  {
    readonly hash: (password: Redacted.Redacted<string>) => Effect.Effect<StoredPassword>;
    readonly verify: (
      password: Redacted.Redacted<string>,
      stored: StoredPassword,
    ) => Effect.Effect<boolean>;
    readonly decoy: Effect.Effect<StoredPassword>;
  }
>()("missal/Passwords") {
  static readonly layer = Layer.effect(
    Passwords,
    Effect.gen(function* () {
      const kdf = yield* Kdf.Kdf;
      const random = yield* SecureRandom.SecureRandom;

      const derive = (password: Redacted.Redacted<string>, salt: Uint8Array, iterations: number) =>
        kdf
          .derive(
            Kdf.pbkdf2({ hash: Digest.sha256, iterations }),
            encoder.encode(Redacted.value(password)),
            { salt, length: HASH_BYTES },
          )
          .pipe(Effect.orDie);

      const hash = Effect.fn("Passwords.hash")(function* (password: Redacted.Redacted<string>) {
        const salt = yield* random.randomBytes(SALT_BYTES);
        const derived = yield* derive(password, salt, ITERATIONS);
        return {
          hash: Base64Url.encode(derived),
          salt: Base64Url.encode(salt),
          iterations: ITERATIONS,
        };
      });

      const verify = Effect.fn("Passwords.verify")(function* (
        password: Redacted.Redacted<string>,
        stored: StoredPassword,
      ) {
        const salt = Base64Url.decode(stored.salt);
        const expected = Base64Url.decode(stored.hash);
        if (Result.isFailure(salt) || Result.isFailure(expected)) return false;
        const derived = yield* derive(password, salt.success, stored.iterations);
        return Mac.timingSafeEqual(derived, expected.success);
      });

      const decoy = yield* Effect.cached(
        hash(Redacted.make("decoy password for unknown accounts")),
      );

      return Passwords.of({ hash, verify, decoy });
    }),
  );
}

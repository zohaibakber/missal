import * as Digest from "effect/crypto/Digest";
import * as Kdf from "effect/crypto/Kdf";
import * as Password from "effect/crypto/Password";

export const ITERATIONS = 100_000;

const algorithm = Kdf.pbkdf2({ hash: Digest.sha256, iterations: ITERATIONS });

export const DECOY = Password.encode({
  algorithm,
  salt: new Uint8Array(16),
  hash: new Uint8Array(32),
});

export const PasswordLive = Password.layer({ algorithm });

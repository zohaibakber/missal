import * as Context from "effect/Context";
import * as Digest from "effect/crypto/Digest";
import * as Mac from "effect/crypto/Mac";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as DateTime from "effect/DateTime";
import * as Duration from "effect/Duration";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlSchema from "effect/sql/SqlSchema";
import { InvalidCode } from "./Api.ts";
import { Atomic } from "./Database.ts";
import { storage } from "./Failures.ts";
import { type MailMessage, Mailer } from "./Mailer.ts";
import type { CodePurpose, Email } from "./Models.ts";

const LIFETIME = Duration.minutes(10);
const COOLDOWN = Duration.seconds(60);
const MAX_ATTEMPTS = 5;

const CodeRow = Schema.Struct({ codeHash: Schema.String, expiresAt: Schema.Number });
const CodeKey = Schema.Struct({ email: Schema.String, purpose: Schema.String });

const encoder = new TextEncoder();

const millis = DateTime.toEpochMillis;

export const wrongCode = () => new InvalidCode({ error: "that code is wrong or has expired" });

const codeMessage = (to: Email, purpose: CodePurpose, code: string): MailMessage => ({
  to,
  subject: purpose === "verify" ? "Confirm your email for Missal" : "Reset your Missal password",
  text: [
    purpose === "verify"
      ? `Your code to confirm your email for Missal is ${code}.`
      : `Your code to reset your Missal password is ${code}.`,
    "It expires in 10 minutes.",
    "If you did not ask for this code, ignore this email.",
  ].join("\n\n"),
});

const accountExistsMessage = (to: Email): MailMessage => ({
  to,
  subject: "Your Missal account",
  text: [
    "Someone tried to create a Missal account with this email address, but it already has one.",
    "If that was you, sign in, or choose Forgot password on the sign-in screen.",
    "If it was not you, ignore this email.",
  ].join("\n\n"),
});

export class Codes extends Context.Service<Codes>()("Codes", {
  make: Effect.gen(function* () {
    const sql = yield* SqlClient.SqlClient;
    const atomic = yield* Atomic;
    const mailer = yield* Mailer;
    const random = yield* SecureRandom.SecureRandom;
    const digest = yield* Digest.Digest;

    const findCode = SqlSchema.findOneOption({
      Request: CodeKey,
      Result: CodeRow,
      execute: ({ email, purpose }) =>
        sql`SELECT code_hash, expires_at FROM email_codes WHERE email = ${email} AND purpose = ${purpose}`,
    });

    const hashCode = (email: Email, purpose: CodePurpose, code: string) =>
      digest.digestHex(Digest.sha256, encoder.encode(`${purpose}:${email}:${code}`));

    const sendMail = (message: MailMessage) => mailer.send(message).pipe(storage("send the email"));

    const claim = Effect.fnUntraced(function* (
      email: Email,
      purpose: CodePurpose | "notice",
      codeHash: string,
      lifetime: Duration.Duration,
    ) {
      const now = millis(yield* DateTime.now);
      const claimed = yield* sql`INSERT INTO email_codes ${sql.insert({
        email,
        purpose,
        codeHash,
        expiresAt: now + Duration.toMillis(lifetime),
        attempts: 0,
        sentAt: now,
      })}
        ON CONFLICT (email, purpose) DO UPDATE SET
          code_hash = excluded.code_hash,
          expires_at = excluded.expires_at,
          attempts = 0,
          sent_at = excluded.sent_at
        WHERE email_codes.sent_at <= ${now - Duration.toMillis(COOLDOWN)}
        RETURNING email`.pipe(storage("store the code"));
      return claimed.length > 0;
    });

    const notice = Effect.fnUntraced(function* (email: Email) {
      if (!(yield* claim(email, "notice", "", COOLDOWN))) return;
      yield* sendMail(accountExistsMessage(email));
    });

    const send = Effect.fnUntraced(function* (email: Email, purpose: CodePurpose) {
      const code = String(yield* random.intBetween(0, 999_999)).padStart(6, "0");
      const codeHash = yield* hashCode(email, purpose, code);
      if (!(yield* claim(email, purpose, codeHash, LIFETIME))) return;
      yield* sendMail(codeMessage(email, purpose, code)).pipe(
        Effect.tapError(() =>
          sql`DELETE FROM email_codes WHERE email = ${email} AND purpose = ${purpose} AND code_hash = ${codeHash}`.pipe(
            Effect.ignore,
          ),
        ),
      );
    });

    const redeem = Effect.fnUntraced(function* (email: Email, purpose: CodePurpose, code: string) {
      const now = millis(yield* DateTime.now);
      const row = yield* findCode({ email, purpose }).pipe(storage("check the code"));
      if (Option.isNone(row)) return yield* wrongCode();
      const { codeHash, expiresAt } = row.value;
      const key = sql`email = ${email} AND purpose = ${purpose} AND code_hash = ${codeHash}`;
      if (expiresAt <= now) {
        yield* sql`DELETE FROM email_codes WHERE ${key}`.pipe(storage("check the code"));
        return yield* wrongCode();
      }
      const presented = encoder.encode(yield* hashCode(email, purpose, code));
      if (Mac.timingSafeEqual(presented, encoder.encode(codeHash))) {
        const used = yield* sql`DELETE FROM email_codes WHERE ${key} RETURNING email`.pipe(
          storage("use the code"),
        );
        if (used.length === 0) return yield* wrongCode();
        return;
      }
      yield* atomic
        .run([
          sql`UPDATE email_codes SET attempts = attempts + 1 WHERE ${key}`,
          sql`DELETE FROM email_codes WHERE ${key} AND attempts >= ${MAX_ATTEMPTS}`,
        ])
        .pipe(storage("check the code"));
      return yield* wrongCode();
    });

    const discard = (email: string, purpose?: CodePurpose) =>
      purpose === undefined
        ? sql`DELETE FROM email_codes WHERE email = ${email}`
        : sql`DELETE FROM email_codes WHERE email = ${email} AND purpose = ${purpose}`;

    const purge = (now: DateTime.Utc) =>
      sql`DELETE FROM email_codes WHERE expires_at <= ${millis(now)}`;

    return { notice, send, redeem, discard, purge };
  }),
}) {
  static readonly layer = Layer.effect(Codes, Codes.make);
}

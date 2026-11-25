import { and, eq, gte, lte, sql } from "drizzle-orm";
import * as Context from "effect/Context";
import * as Digest from "effect/crypto/Digest";
import * as Mac from "effect/crypto/Mac";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as DateTime from "effect/DateTime";
import * as Duration from "effect/Duration";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import { InvalidCode } from "./Api.ts";
import { Atomic, Db } from "./Database.ts";
import { storage } from "./Failures.ts";
import { type MailMessage, Mailer } from "./Mailer.ts";
import type { CodePurpose, Email } from "./Models.ts";
import { emailCodes } from "./Schema.ts";

const LIFETIME = Duration.minutes(10);
const COOLDOWN = Duration.seconds(60);
const MAX_ATTEMPTS = 5;

const encoder = new TextEncoder();

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
    const db = yield* Db;
    const atomic = yield* Atomic;
    const mailer = yield* Mailer;
    const random = yield* SecureRandom.SecureRandom;
    const digest = yield* Digest.Digest;

    const keyOf = (email: string, purpose: CodePurpose | "notice") =>
      and(eq(emailCodes.email, email), eq(emailCodes.purpose, purpose));

    const hashCode = (email: Email, purpose: CodePurpose, code: string) =>
      digest.digestHex(Digest.sha256, encoder.encode(`${purpose}:${email}:${code}`));

    const sendMail = (message: MailMessage) => mailer.send(message).pipe(storage("send the email"));

    const claim = Effect.fnUntraced(function* (
      email: Email,
      purpose: CodePurpose | "notice",
      codeHash: string,
      lifetime: Duration.Duration,
    ) {
      const now = DateTime.toEpochMillis(yield* DateTime.now);
      const claimed = yield* db
        .insert(emailCodes)
        .values({
          email,
          purpose,
          codeHash,
          expiresAt: now + Duration.toMillis(lifetime),
          attempts: 0,
          sentAt: now,
        })
        .onConflictDoUpdate({
          target: [emailCodes.email, emailCodes.purpose],
          set: {
            codeHash: sql`excluded.code_hash`,
            expiresAt: sql`excluded.expires_at`,
            attempts: 0,
            sentAt: sql`excluded.sent_at`,
          },
          setWhere: lte(emailCodes.sentAt, now - Duration.toMillis(COOLDOWN)),
        })
        .returning({ email: emailCodes.email })
        .pipe(storage("store the code"));
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
          db
            .delete(emailCodes)
            .where(and(keyOf(email, purpose), eq(emailCodes.codeHash, codeHash)))
            .pipe(Effect.ignore),
        ),
      );
    });

    const redeem = Effect.fnUntraced(function* (email: Email, purpose: CodePurpose, code: string) {
      const now = DateTime.toEpochMillis(yield* DateTime.now);
      const [row] = yield* db
        .select({ codeHash: emailCodes.codeHash, expiresAt: emailCodes.expiresAt })
        .from(emailCodes)
        .where(keyOf(email, purpose))
        .pipe(storage("check the code"));
      if (row === undefined) return yield* wrongCode();
      const { codeHash, expiresAt } = row;
      const key = and(keyOf(email, purpose), eq(emailCodes.codeHash, codeHash));
      if (expiresAt <= now) {
        yield* db.delete(emailCodes).where(key).pipe(storage("check the code"));
        return yield* wrongCode();
      }
      const presented = encoder.encode(yield* hashCode(email, purpose, code));
      if (Mac.timingSafeEqual(presented, encoder.encode(codeHash))) {
        const used = yield* db
          .delete(emailCodes)
          .where(key)
          .returning({ email: emailCodes.email })
          .pipe(storage("use the code"));
        if (used.length === 0) return yield* wrongCode();
        return;
      }
      yield* atomic
        .run([
          db
            .update(emailCodes)
            .set({ attempts: sql`${emailCodes.attempts} + 1` })
            .where(key),
          db.delete(emailCodes).where(and(key, gte(emailCodes.attempts, MAX_ATTEMPTS))),
        ])
        .pipe(storage("check the code"));
      return yield* wrongCode();
    });

    const discard = (email: string, purpose?: CodePurpose) =>
      db
        .delete(emailCodes)
        .where(purpose === undefined ? eq(emailCodes.email, email) : keyOf(email, purpose));

    const purge = (now: DateTime.Utc) =>
      db.delete(emailCodes).where(lte(emailCodes.expiresAt, DateTime.toEpochMillis(now)));

    return { notice, send, redeem, discard, purge };
  }),
}) {
  static readonly layer = Layer.effect(Codes, Codes.make);
}

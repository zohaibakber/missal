import type { RateLimitClient } from "@alchemy.run/cloudflare/Workers/RateLimit";
import type { RuntimeContext } from "alchemy/RuntimeContext";
import * as Context from "effect/Context";
import * as DateTime from "effect/DateTime";
import * as Duration from "effect/Duration";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlSchema from "effect/sql/SqlSchema";
import { AccountLocked } from "./Api.ts";
import { storage } from "./Failures.ts";

export const WINDOW = Duration.seconds(60);
const LOCK_LIFETIME = Duration.minutes(15);
const MAX_FAILURES = 5;

export const tooManyAttempts = (retryAfter: Duration.Duration) =>
  new AccountLocked({
    error: "too many attempts from this network; wait a minute",
    retryAfter: Math.max(1, Math.ceil(Duration.toSeconds(retryAfter))),
  });

export class AuthThrottle extends Context.Service<
  AuthThrottle,
  { readonly check: (client: string) => Effect.Effect<void, AccountLocked, RuntimeContext> }
>()("AuthThrottle") {
  static readonly layer = (limiter: RateLimitClient) =>
    Layer.succeed(
      AuthThrottle,
      AuthThrottle.of({
        check: Effect.fn("AuthThrottle.check")(function* (client: string) {
          const outcome = yield* limiter.limit({ key: `auth:${client}` }).pipe(
            Effect.tapError((cause) => Effect.logWarning("rate limiter unavailable", cause)),
            Effect.orElseSucceed(() => ({ success: true })),
          );
          if (!outcome.success) return yield* tooManyAttempts(WINDOW);
        }),
      }),
    );
}

const FailureRow = Schema.Struct({ failures: Schema.Int, lockedUntil: Schema.Number });

const millis = DateTime.toEpochMillis;

export class SignInLockout extends Context.Service<SignInLockout>()("SignInLockout", {
  make: Effect.gen(function* () {
    const sql = yield* SqlClient.SqlClient;

    const findFailures = SqlSchema.findOneOption({
      Request: Schema.String,
      Result: FailureRow,
      execute: (email) =>
        sql`SELECT failures, locked_until FROM sign_in_failures WHERE email = ${email}`,
    });

    const check = Effect.fnUntraced(function* (email: string, now: DateTime.Utc) {
      const lock = yield* findFailures(email).pipe(storage("check sign-in attempts"));
      if (Option.isSome(lock) && lock.value.lockedUntil > millis(now)) {
        return yield* new AccountLocked({
          error: "too many failed sign-ins; try again later",
          retryAfter: Math.ceil((lock.value.lockedUntil - millis(now)) / 1000),
        });
      }
    });

    const recordFailure = (email: string, now: DateTime.Utc) =>
      sql`INSERT INTO sign_in_failures (email, failures, locked_until) VALUES (${email}, 1, 0)
          ON CONFLICT (email) DO UPDATE SET
            failures = CASE WHEN failures + 1 >= ${MAX_FAILURES} THEN 0 ELSE failures + 1 END,
            locked_until = CASE WHEN failures + 1 >= ${MAX_FAILURES} THEN ${millis(now) + Duration.toMillis(LOCK_LIFETIME)} ELSE locked_until END`.pipe(
        storage("record a failed sign-in"),
      );

    const clear = (email: string) => sql`DELETE FROM sign_in_failures WHERE email = ${email}`;

    const purge = (now: DateTime.Utc) =>
      sql`DELETE FROM sign_in_failures WHERE locked_until <= ${millis(DateTime.subtract(now, { days: 1 }))}`;

    return { check, recordFailure, clear, purge };
  }),
}) {
  static readonly layer = Layer.effect(SignInLockout, SignInLockout.make);
}

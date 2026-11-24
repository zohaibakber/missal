import type { RateLimitClient } from "@alchemy.run/cloudflare/Workers/RateLimit";
import { RuntimeContext } from "alchemy/RuntimeContext";
import { eq, lte, sql } from "drizzle-orm";
import * as Context from "effect/Context";
import * as DateTime from "effect/DateTime";
import * as Duration from "effect/Duration";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import { AccountLocked } from "./Api.ts";
import { Db } from "./Database.ts";
import { storage } from "./Failures.ts";
import { signInFailures } from "./Schema.ts";

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
  { readonly check: (client: string) => Effect.Effect<void, AccountLocked> }
>()("AuthThrottle") {
  static readonly layer = (limiter: RateLimitClient) =>
    Layer.succeed(
      AuthThrottle,
      AuthThrottle.of({
        check: Effect.fn("AuthThrottle.check")(function* (client: string) {
          const outcome = yield* limiter.limit({ key: `auth:${client}` }).pipe(
            Effect.provide(RuntimeContext.phantom),
            Effect.tapError((cause) => Effect.logWarning("rate limiter unavailable", cause)),
            Effect.orElseSucceed(() => ({ success: true })),
          );
          if (!outcome.success) return yield* tooManyAttempts(WINDOW);
        }),
      }),
    );
}

const millis = DateTime.toEpochMillis;

export class SignInLockout extends Context.Service<SignInLockout>()("SignInLockout", {
  make: Effect.gen(function* () {
    const db = yield* Db;

    const check = Effect.fnUntraced(function* (email: string, now: DateTime.Utc) {
      const [lock] = yield* db
        .select({ lockedUntil: signInFailures.lockedUntil })
        .from(signInFailures)
        .where(eq(signInFailures.email, email))
        .pipe(storage("check sign-in attempts"));
      if (lock !== undefined && lock.lockedUntil > millis(now)) {
        return yield* new AccountLocked({
          error: "too many failed sign-ins; try again later",
          retryAfter: Math.ceil((lock.lockedUntil - millis(now)) / 1000),
        });
      }
    });

    const recordFailure = (email: string, now: DateTime.Utc) => {
      const lockedUntil = millis(now) + Duration.toMillis(LOCK_LIFETIME);
      const reachesLimit = sql`${signInFailures.failures} + 1 >= ${MAX_FAILURES}`;
      return db
        .insert(signInFailures)
        .values({ email, failures: 1, lockedUntil: 0 })
        .onConflictDoUpdate({
          target: signInFailures.email,
          set: {
            failures: sql`CASE WHEN ${reachesLimit} THEN 0 ELSE ${signInFailures.failures} + 1 END`,
            lockedUntil: sql`CASE WHEN ${reachesLimit} THEN ${lockedUntil} ELSE ${signInFailures.lockedUntil} END`,
          },
        })
        .pipe(storage("record a failed sign-in"));
    };

    const clear = (email: string) =>
      db.delete(signInFailures).where(eq(signInFailures.email, email));

    const purge = (now: DateTime.Utc) =>
      db
        .delete(signInFailures)
        .where(lte(signInFailures.lockedUntil, millis(DateTime.subtract(now, { days: 1 }))));

    return { check, recordFailure, clear, purge };
  }),
}) {
  static readonly layer = Layer.effect(SignInLockout, SignInLockout.make);
}

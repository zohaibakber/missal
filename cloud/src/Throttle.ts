import type { RateLimitClient } from "@alchemy.run/cloudflare/Workers/RateLimit";
import type { RuntimeContext } from "alchemy/RuntimeContext";
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import { AccountLocked } from "./Api.ts";

export const WINDOW_SECONDS = 60;

const slowDown = () =>
  new AccountLocked({
    error: "too many sign-in attempts from this network; wait a minute",
    retryAfter: WINDOW_SECONDS,
  });

export class SignInThrottle extends Context.Service<
  SignInThrottle,
  { readonly check: (client: string) => Effect.Effect<void, AccountLocked, RuntimeContext> }
>()("missal/SignInThrottle") {
  static readonly layer = (limiter: RateLimitClient) =>
    Layer.succeed(
      SignInThrottle,
      SignInThrottle.of({
        check: Effect.fn("SignInThrottle.check")(function* (client: string) {
          const outcome = yield* limiter.limit({ key: `sign-in:${client}` }).pipe(
            Effect.tapError((cause) => Effect.logWarning("rate limiter unavailable", cause)),
            Effect.orElseSucceed(() => ({ success: true })),
          );
          if (!outcome.success) return yield* slowDown();
        }),
      }),
    );

  static readonly layerMemory = (limit: number) =>
    Layer.sync(SignInThrottle, () => {
      const counts = new Map<string, number>();
      return SignInThrottle.of({
        check: (client) =>
          Effect.suspend(() => {
            const count = (counts.get(client) ?? 0) + 1;
            counts.set(client, count);
            return count > limit ? Effect.fail(slowDown()) : Effect.void;
          }),
      });
    });
}

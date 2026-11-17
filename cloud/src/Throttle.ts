import type { RateLimitClient } from "@alchemy.run/cloudflare/Workers/RateLimit";
import type { RuntimeContext } from "alchemy/RuntimeContext";
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import { AccountLocked } from "./Api.ts";

export const WINDOW_SECONDS = 60;

const slowDown = () =>
  new AccountLocked({
    error: "too many attempts from this network; wait a minute",
    retryAfter: WINDOW_SECONDS,
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
          if (!outcome.success) return yield* slowDown();
        }),
      }),
    );

  static readonly layerMemory = (limit: number) =>
    Layer.sync(AuthThrottle, () => {
      const counts = new Map<string, number>();
      return AuthThrottle.of({
        check: (client) =>
          Effect.suspend(() => {
            const count = (counts.get(client) ?? 0) + 1;
            counts.set(client, count);
            return count > limit ? Effect.fail(slowDown()) : Effect.void;
          }),
      });
    });
}

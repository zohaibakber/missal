import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import {
  AdminsOnly,
  Authentication,
  canPublish,
  NotAdmin,
  NotPublisher,
  PasswordChangeRequired,
  PasswordCurrent,
  PublishersOnly,
  Viewer,
} from "./Api.ts";
import { Tokens } from "./Tokens.ts";

export const AuthenticationLive = Layer.effect(
  Authentication,
  Effect.gen(function* () {
    const tokens = yield* Tokens;
    return Authentication.of({
      bearer: Effect.fn("Authentication.bearer")(function* (httpEffect, { credential }) {
        const viewer = yield* tokens.verify(credential);
        return yield* Effect.provideService(httpEffect, Viewer, viewer);
      }),
    });
  }),
);

export const PasswordCurrentLive = Layer.succeed(
  PasswordCurrent,
  PasswordCurrent.of(
    Effect.fn("PasswordCurrent")(function* (httpEffect) {
      const viewer = yield* Viewer;
      if (viewer.mustChangePassword) {
        return yield* new PasswordChangeRequired({ error: "choose a new password first" });
      }
      return yield* httpEffect;
    }),
  ),
);

export const PublishersOnlyLive = Layer.succeed(
  PublishersOnly,
  PublishersOnly.of(
    Effect.fn("PublishersOnly")(function* (httpEffect) {
      const viewer = yield* Viewer;
      if (!canPublish(viewer.role)) {
        return yield* new NotPublisher({ error: "this account cannot publish templates" });
      }
      return yield* httpEffect;
    }),
  ),
);

export const AdminsOnlyLive = Layer.succeed(
  AdminsOnly,
  AdminsOnly.of(
    Effect.fn("AdminsOnly")(function* (httpEffect) {
      const viewer = yield* Viewer;
      if (viewer.role !== "admin") {
        return yield* new NotAdmin({ error: "only administrators can manage accounts" });
      }
      return yield* httpEffect;
    }),
  ),
);

export const AuthorizationLive = Layer.mergeAll(
  AuthenticationLive,
  PasswordCurrentLive,
  PublishersOnlyLive,
  AdminsOnlyLive,
);

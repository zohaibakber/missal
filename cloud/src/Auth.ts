import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import * as SqlSchema from "effect/sql/SqlSchema";
import {
  AdminsOnly,
  Authentication,
  NotAdmin,
  LicenceRequired,
  Licensed,
  NotPublisher,
  PublishersOnly,
  Viewer,
} from "./Api.ts";
import { canPublish } from "./Models.ts";
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

export const LicensedLive = Layer.succeed(
  Licensed,
  Licensed.of(
    Effect.fn("Licensed")(function* (httpEffect) {
      const viewer = yield* Viewer;
      if (DateTime.isLessThanOrEqualTo(viewer.licensedUntil, yield* DateTime.now)) {
        return yield* new LicenceRequired({ error: "your Missal licence has ended" });
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

const AdminRow = Schema.Struct({ role: Schema.String, disabled: Schema.Number });

export const AdminsOnlyLive = Layer.effect(
  AdminsOnly,
  Effect.gen(function* () {
    const sql = yield* SqlClient.SqlClient;
    const current = SqlSchema.findOneOption({
      Request: Schema.String,
      Result: AdminRow,
      execute: (id) => sql`SELECT role, disabled FROM users WHERE id = ${id}`,
    });
    const refused = () => new NotAdmin({ error: "only administrators can manage accounts" });
    return AdminsOnly.of(
      Effect.fn("AdminsOnly")(function* (httpEffect) {
        const viewer = yield* Viewer;
        if (viewer.role !== "admin") return yield* refused();
        const row = yield* current(viewer.id).pipe(
          Effect.tapError((cause) => Effect.logError("check the administrator failed", cause)),
          Effect.orElseSucceed(() => Option.none()),
        );
        const stillAdmin = Option.exists(
          row,
          (user) => user.role === "admin" && user.disabled === 0,
        );
        if (!stillAdmin) return yield* refused();
        return yield* httpEffect;
      }),
    );
  }),
);

export const AuthorizationLive = Layer.mergeAll(
  AuthenticationLive,
  LicensedLive,
  PublishersOnlyLive,
  AdminsOnlyLive,
);

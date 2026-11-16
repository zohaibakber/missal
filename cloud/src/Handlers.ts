import * as Effect from "effect/Effect";
import * as HttpApiBuilder from "effect/http-api/HttpApiBuilder";
import * as HttpApiSchema from "effect/http-api/HttpApiSchema";
import * as HttpServerRequest from "effect/http/HttpServerRequest";
import * as Layer from "effect/Layer";
import { canPublish, MissalApi, Viewer } from "./Api.ts";
import { Accounts } from "./Accounts.ts";
import { Library } from "./Library.ts";

const AuthHandlers = HttpApiBuilder.group(
  MissalApi,
  "auth",
  Effect.fn(function* (handlers) {
    const accounts = yield* Accounts;
    return handlers.handleAll({
      signIn: ({ payload }) =>
        Effect.flatMap(HttpServerRequest.HttpServerRequest, (request) =>
          accounts.signIn(
            request.headers["cf-connecting-ip"] ?? "unknown",
            payload.email,
            payload.password,
          ),
        ),
      refresh: ({ payload }) => accounts.refresh(payload.refreshToken),
      signOut: ({ payload }) => accounts.signOut(payload.refreshToken),
    });
  }),
);

const AccountHandlers = HttpApiBuilder.group(
  MissalApi,
  "account",
  Effect.fn(function* (handlers) {
    const accounts = yield* Accounts;
    return handlers.handleAll({
      me: () => Effect.flatMap(Viewer, accounts.me),
      changePassword: ({ payload }) =>
        Effect.flatMap(Viewer, (viewer) =>
          accounts.changePassword(viewer, payload.currentPassword, payload.newPassword),
        ),
    });
  }),
);

const CatalogHandlers = HttpApiBuilder.group(
  MissalApi,
  "catalog",
  Effect.fn(function* (handlers) {
    const library = yield* Library;
    return handlers.handleAll({
      list: () =>
        Effect.gen(function* () {
          const viewer = yield* Viewer;
          const templates = yield* library.list;
          return { publisher: canPublish(viewer.role), templates };
        }),
      download: ({ params }) =>
        library.download(params.id).pipe(
          Effect.map((stored) =>
            HttpApiSchema.withHeaders({
              body: stored.content,
              headers: { "x-missal-version": stored.version },
            }),
          ),
        ),
    });
  }),
);

const PublishingHandlers = HttpApiBuilder.group(
  MissalApi,
  "publishing",
  Effect.fn(function* (handlers) {
    const library = yield* Library;
    return handlers.handleAll({
      create: ({ payload }) => Effect.flatMap(Viewer, (viewer) => library.create(payload, viewer.id)),
      update: ({ params, payload }) =>
        Effect.flatMap(Viewer, (viewer) => library.update(params.id, payload, viewer.id)),
      remove: ({ params }) => library.remove(params.id),
    });
  }),
);

const UsersHandlers = HttpApiBuilder.group(
  MissalApi,
  "users",
  Effect.fn(function* (handlers) {
    const accounts = yield* Accounts;
    return handlers.handleAll({
      list: () => accounts.list,
      create: ({ payload }) => accounts.create(payload),
      update: ({ params, payload }) =>
        Effect.flatMap(Viewer, (viewer) => accounts.update(viewer, params.id, payload)),
      resetPassword: ({ params, payload }) => accounts.resetPassword(params.id, payload.password),
    });
  }),
);

const HealthHandlers = HttpApiBuilder.group(
  MissalApi,
  "health",
  Effect.fn(function* (handlers) {
    return handlers.handleAll({ health: () => Effect.succeed("ok") });
  }),
);

export const HandlersLive = Layer.mergeAll(
  AuthHandlers,
  AccountHandlers,
  CatalogHandlers,
  PublishingHandlers,
  UsersHandlers,
  HealthHandlers,
);

import * as Effect from "effect/Effect";
import * as HttpApiBuilder from "effect/http-api/HttpApiBuilder";
import * as HttpApiSchema from "effect/http-api/HttpApiSchema";
import * as HttpServerRequest from "effect/http/HttpServerRequest";
import * as Layer from "effect/Layer";
import { canPublish, Api, Viewer } from "./Api.ts";
import { Accounts } from "./Accounts.ts";
import { Library } from "./Library.ts";

const clientAddress = Effect.map(
  HttpServerRequest.HttpServerRequest,
  (request) => request.headers["cf-connecting-ip"] ?? "unknown",
);

const AuthHandlers = HttpApiBuilder.group(
  Api,
  "auth",
  Effect.fn(function* (handlers) {
    const accounts = yield* Accounts;
    return handlers.handleAll({
      signUp: ({ payload }) =>
        Effect.flatMap(clientAddress, (client) => accounts.signUp(client, payload)),
      requestCode: ({ payload }) =>
        Effect.flatMap(clientAddress, (client) =>
          accounts.requestCode(client, payload.email, payload.purpose),
        ),
      verifyEmail: ({ payload }) =>
        Effect.flatMap(clientAddress, (client) =>
          accounts.verifyEmail(client, payload.email, payload.code),
        ),
      resetPassword: ({ payload }) =>
        Effect.flatMap(clientAddress, (client) =>
          accounts.resetPassword(client, payload.email, payload.code, payload.password),
        ),
      signIn: ({ payload }) =>
        Effect.flatMap(clientAddress, (client) =>
          accounts.signIn(client, payload.email, payload.password),
        ),
      refresh: ({ payload }) => accounts.refresh(payload.refreshToken),
      signOut: ({ payload }) => accounts.signOut(payload.refreshToken),
    });
  }),
);

const AccountHandlers = HttpApiBuilder.group(
  Api,
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
  Api,
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
  Api,
  "publishing",
  Effect.fn(function* (handlers) {
    const library = yield* Library;
    return handlers.handleAll({
      create: ({ payload }) =>
        Effect.flatMap(Viewer, (viewer) => library.create(payload, viewer.id)),
      update: ({ params, payload }) =>
        Effect.flatMap(Viewer, (viewer) => library.update(params.id, payload, viewer.id)),
      remove: ({ params }) => library.remove(params.id),
    });
  }),
);

const UsersHandlers = HttpApiBuilder.group(
  Api,
  "users",
  Effect.fn(function* (handlers) {
    const accounts = yield* Accounts;
    return handlers.handleAll({
      list: () => accounts.list,
      update: ({ params, payload }) =>
        Effect.flatMap(Viewer, (viewer) => accounts.update(viewer, params.id, payload)),
      grantLicence: ({ params, payload }) => accounts.grantLicence(params.id, payload.years),
    });
  }),
);

const HealthHandlers = HttpApiBuilder.group(Api, "health", (handlers) =>
  handlers.handleAll({ health: () => Effect.succeed("ok") }),
);

export const HandlersLive = Layer.mergeAll(
  AuthHandlers,
  AccountHandlers,
  CatalogHandlers,
  PublishingHandlers,
  UsersHandlers,
  HealthHandlers,
);

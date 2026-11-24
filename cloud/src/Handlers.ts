import * as Effect from "effect/Effect";
import * as HttpApiBuilder from "effect/http-api/HttpApiBuilder";
import * as HttpApiSchema from "effect/http-api/HttpApiSchema";
import * as Layer from "effect/Layer";
import { Api, Viewer } from "./Api.ts";
import { Accounts } from "./Accounts.ts";
import { Library } from "./Library.ts";
import { canPublish } from "./Models.ts";
import { People } from "./People.ts";
import { Sessions } from "./Sessions.ts";

const AuthHandlers = HttpApiBuilder.group(
  Api,
  "auth",
  Effect.fn(function* (handlers) {
    const accounts = yield* Accounts;
    const sessions = yield* Sessions;
    return handlers.handleAll({
      signUp: ({ payload }) => accounts.signUp(payload),
      requestCode: ({ payload }) => accounts.requestCode(payload.email, payload.purpose),
      verifyEmail: ({ payload }) => accounts.verifyEmail(payload.email, payload.code),
      resetPassword: ({ payload }) =>
        accounts.resetPassword(payload.email, payload.code, payload.password),
      signIn: ({ payload }) => accounts.signIn(payload.email, payload.password),
      refresh: ({ payload }) => sessions.refresh(payload.refreshToken),
      signOut: ({ payload }) => sessions.revoke(payload.refreshToken),
    });
  }),
);

const AccountHandlers = HttpApiBuilder.group(
  Api,
  "account",
  Effect.fn(function* (handlers) {
    const accounts = yield* Accounts;
    return handlers.handleAll({
      me: () => accounts.me,
      changePassword: ({ payload }) =>
        accounts.changePassword(payload.currentPassword, payload.newPassword),
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
      create: ({ payload }) => library.create(payload),
      update: ({ params, payload }) => library.update(params.id, payload),
      remove: ({ params }) => library.remove(params.id),
    });
  }),
);

const TemplateAdminHandlers = HttpApiBuilder.group(
  Api,
  "templateAdmin",
  Effect.fn(function* (handlers) {
    const library = yield* Library;
    return handlers.handleAll({
      list: () => library.managed,
      versions: ({ params }) => library.versions(params.id),
      restore: ({ params, payload }) => library.restore(params.id, payload.version),
    });
  }),
);

const UsersHandlers = HttpApiBuilder.group(
  Api,
  "users",
  Effect.fn(function* (handlers) {
    const people = yield* People;
    return handlers.handleAll({
      list: () => people.list,
      create: ({ payload }) => people.create(payload),
      update: ({ params, payload }) => people.update(params.id, payload),
      remove: ({ params }) => people.remove(params.id),
      verifyEmail: ({ params }) => people.confirmEmail(params.id),
      grantLicence: ({ params, payload }) => people.grantLicence(params.id, payload.months),
      endLicence: ({ params }) => people.endLicence(params.id),
      extendTrial: ({ params, payload }) => people.extendTrial(params.id, payload.days),
      setPassword: ({ params, payload }) => people.setPassword(params.id, payload.password),
      sendReset: ({ params }) => people.sendReset(params.id),
      signOut: ({ params }) => people.endSessions(params.id),
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
  TemplateAdminHandlers,
  UsersHandlers,
  HealthHandlers,
);

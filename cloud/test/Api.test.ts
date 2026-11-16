import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "@effect/vitest";
import * as SqliteClient from "@effect/sql-sqlite-node/SqliteClient";
import * as Http from "alchemy/Http";
import { RuntimeContext } from "alchemy/RuntimeContext";
import * as ConfigProvider from "effect/ConfigProvider";
import * as Context from "effect/Context";
import * as DateTime from "effect/DateTime";
import * as Effect from "effect/Effect";
import * as HttpApiMiddleware from "effect/http-api/HttpApiMiddleware";
import * as HttpApiTest from "effect/http-api/HttpApiTest";
import * as HttpClientRequest from "effect/http/HttpClientRequest";
import * as HttpRouter from "effect/http/HttpRouter";
import * as Layer from "effect/Layer";
import * as Redacted from "effect/Redacted";
import * as Ref from "effect/Ref";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import { Accounts } from "../src/Accounts.ts";
import { Account, Authentication, MissalApi } from "../src/Api.ts";
import { CryptoLive, ServicesLive } from "../src/App.ts";
import { Email } from "../src/Models.ts";
import { AuthorizationLive } from "../src/Auth.ts";
import { Blobs } from "../src/Blobs.ts";
import { Atomic, columnNames } from "../src/Database.ts";
import { HandlersLive } from "../src/Handlers.ts";
import { SignInThrottle } from "../src/Throttle.ts";

const ADMIN = "admin@missal.test";
const BOOTSTRAP = "bootstrap-pass-1";
const ADMIN_PASSWORD = "admin-secret-123";

class CurrentToken extends Context.Service<CurrentToken, Ref.Ref<string | undefined>>()(
  "test/CurrentToken",
) {
  static readonly layer = Layer.effect(CurrentToken, Ref.make<string | undefined>(undefined));
}

const AuthenticationClient = HttpApiMiddleware.layerClient(
  Authentication,
  Effect.fn(function* ({ next, request }) {
    const token = yield* Ref.get(yield* CurrentToken);
    return yield* next(
      token === undefined ? request : HttpClientRequest.bearerToken(request, token),
    );
  }),
);

const migrate = Layer.effectDiscard(
  Effect.gen(function* () {
    const sql = yield* SqlClient.SqlClient;
    const directory = join(import.meta.dirname, "..", "migrations");
    for (const file of readdirSync(directory).sort()) {
      const statements = readFileSync(join(directory, file), "utf8")
        .split(";")
        .map((statement) => statement.trim())
        .filter((statement) => statement.length > 0);
      for (const statement of statements) yield* sql.unsafe(statement);
    }
  }),
);

const testLayer = (throttleLimit = 1_000) => {
  const database = Layer.mergeAll(Atomic.layerTransaction, migrate).pipe(
    Layer.provideMerge(SqliteClient.layer({ filename: ":memory:", ...columnNames })),
  );
  const infrastructure = Layer.mergeAll(
    database,
    CryptoLive,
    SignInThrottle.layerMemory(throttleLimit),
    Blobs.layerMemory,
  );
  const app = HandlersLive.pipe(
    HttpRouter.provideRequest(RuntimeContext.phantom),
    Layer.provideMerge(AuthorizationLive),
    Layer.provideMerge(ServicesLive({ email: ADMIN, password: Redacted.make(BOOTSTRAP) })),
  );
  return Layer.mergeAll(app, AuthenticationClient).pipe(
    Layer.provideMerge(infrastructure),
    Layer.provideMerge(CurrentToken.layer),
    Layer.provideMerge(Http.Platform),
    Layer.provide(
      ConfigProvider.layer(ConfigProvider.fromUnknown({ MISSAL_TOKEN_SECRET: "ab".repeat(32) })),
    ),
    Layer.provide(RuntimeContext.phantom),
  );
};

const client = HttpApiTest.groups(MissalApi, ["auth", "account", "catalog", "publishing", "users"]);

const useToken = Effect.fn(function* (token: string | undefined) {
  yield* Ref.set(yield* CurrentToken, token);
});

const signIn = (email: string, password: string) =>
  Effect.flatMap(client, (api) =>
    api.auth.signIn({ payload: { email, password: Redacted.make(password) } }),
  );

const adminSession = Effect.gen(function* () {
  const api = yield* client;
  const first = yield* signIn(ADMIN, BOOTSTRAP);
  yield* useToken(first.accessToken);
  const changed = yield* api.account.changePassword({
    payload: {
      currentPassword: Redacted.make(BOOTSTRAP),
      newPassword: Redacted.make(ADMIN_PASSWORD),
    },
  });
  yield* useToken(changed.accessToken);
  return changed;
});

const addPerson = (
  email: string,
  role: "admin" | "publisher" | "member",
  password = "temporary-123",
) =>
  Effect.flatMap(client, (api) =>
    api.users.create({
      payload: { email, name: "", role, password: Redacted.make(password) },
    }),
  );

const signInAs = Effect.fn(function* (email: string, temporary: string, chosen: string) {
  const api = yield* client;
  const first = yield* signIn(email, temporary);
  yield* useToken(first.accessToken);
  const session = yield* api.account.changePassword({
    payload: { currentPassword: Redacted.make(temporary), newPassword: Redacted.make(chosen) },
  });
  yield* useToken(session.accessToken);
  return session;
});

describe("accounts", () => {
  it.live("bootstraps the first administrator once and forces a password change", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const wrong = yield* signIn(ADMIN, "not-the-bootstrap").pipe(Effect.flip);
      expect(wrong._tag).toBe("InvalidCredentials");

      const first = yield* signIn(ADMIN, BOOTSTRAP);
      expect(first.account.role).toBe("admin");
      expect(first.account.email).toBe(ADMIN);
      expect(first.account.mustChangePassword).toBe(true);

      yield* useToken(first.accessToken);
      const blocked = yield* api.catalog.list().pipe(Effect.flip);
      expect(blocked._tag).toBe("PasswordChangeRequired");
      const short = yield* api.account
        .changePassword({
          payload: {
            currentPassword: Redacted.make(BOOTSTRAP),
            newPassword: Redacted.make("short"),
          },
        })
        .pipe(Effect.flip);
      expect(short._tag).not.toBe("WrongPassword");

      const changed = yield* api.account.changePassword({
        payload: {
          currentPassword: Redacted.make(BOOTSTRAP),
          newPassword: Redacted.make(ADMIN_PASSWORD),
        },
      });
      expect(changed.account.mustChangePassword).toBe(false);
      const stale = yield* api.auth
        .refresh({ payload: { refreshToken: first.refreshToken } })
        .pipe(Effect.flip);
      expect(stale._tag).toBe("SessionExpired");

      yield* useToken(changed.accessToken);
      expect((yield* api.catalog.list()).publisher).toBe(true);
      const again = yield* signIn(ADMIN, BOOTSTRAP).pipe(Effect.flip);
      expect(again._tag).toBe("InvalidCredentials");
    }).pipe(Effect.provide(testLayer())),
  );

  it.live("roles gate the API and administrators manage people", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const admin = yield* adminSession;
      const reader = yield* addPerson("reader@missal.test", "member");
      expect(reader.email).toBe("reader@missal.test");
      expect(reader.mustChangePassword).toBe(true);
      expect((yield* addPerson("reader@missal.test", "member").pipe(Effect.flip))._tag).toBe(
        "EmailTaken",
      );
      const own = yield* api.users
        .update({ params: { id: admin.account.id }, payload: { role: "member" } })
        .pipe(Effect.flip);
      expect(own._tag).toBe("OwnAccess");

      yield* signInAs("reader@missal.test", "temporary-123", "reader-pass-123");
      expect((yield* api.users.list().pipe(Effect.flip))._tag).toBe("NotAdmin");
      const upload = { name: "x", fieldCount: 0, preview: "", content: new Uint8Array([1, 2, 3]) };
      expect((yield* api.publishing.create({ payload: upload }).pipe(Effect.flip))._tag).toBe(
        "NotPublisher",
      );
      expect((yield* api.catalog.list()).publisher).toBe(false);

      yield* useToken(admin.accessToken);
      yield* api.users.update({ params: { id: reader.id }, payload: { role: "publisher" } });
      const promoted = yield* signIn("reader@missal.test", "reader-pass-123");
      expect(promoted.account.role).toBe("publisher");

      yield* api.users.update({ params: { id: reader.id }, payload: { disabled: true } });
      const ended = yield* api.auth
        .refresh({ payload: { refreshToken: promoted.refreshToken } })
        .pipe(Effect.flip);
      expect(ended._tag).toBe("SessionExpired");
      expect((yield* signIn("reader@missal.test", "reader-pass-123").pipe(Effect.flip))._tag).toBe(
        "InvalidCredentials",
      );

      yield* api.users.update({ params: { id: reader.id }, payload: { disabled: false } });
      const reset = yield* api.users.resetPassword({
        params: { id: reader.id },
        payload: { password: Redacted.make("fresh-temp-456") },
      });
      expect(reset.mustChangePassword).toBe(true);
      expect(
        (yield* signIn("reader@missal.test", "fresh-temp-456")).account.mustChangePassword,
      ).toBe(true);
      expect((yield* api.users.list()).map((account) => account.email)).toEqual([
        ADMIN,
        "reader@missal.test",
      ]);
    }).pipe(Effect.provide(testLayer())),
  );

  it.live("refresh tokens rotate, a reused one ends the session, and sign-out ends it too", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const admin = yield* adminSession;
      const rotated = yield* api.auth.refresh({ payload: { refreshToken: admin.refreshToken } });
      expect(rotated.refreshToken).not.toBe(admin.refreshToken);
      const reused = yield* api.auth
        .refresh({ payload: { refreshToken: admin.refreshToken } })
        .pipe(Effect.flip);
      expect(reused._tag).toBe("SessionExpired");
      const after = yield* api.auth
        .refresh({ payload: { refreshToken: rotated.refreshToken } })
        .pipe(Effect.flip);
      expect(after._tag).toBe("SessionExpired");

      const fresh = yield* signIn(ADMIN, ADMIN_PASSWORD);
      yield* api.auth.signOut({ payload: { refreshToken: fresh.refreshToken } });
      const gone = yield* api.auth
        .refresh({ payload: { refreshToken: fresh.refreshToken } })
        .pipe(Effect.flip);
      expect(gone._tag).toBe("SessionExpired");

      yield* useToken(`${fresh.accessToken.slice(0, -2)}xx`);
      expect((yield* api.account.me().pipe(Effect.flip))._tag).toBe("Unauthorized");
    }).pipe(Effect.provide(testLayer())),
  );

  it.live("five wrong passwords lock the account", () =>
    Effect.gen(function* () {
      yield* adminSession;
      for (let attempt = 0; attempt < 5; attempt++) {
        expect((yield* signIn(ADMIN, "wrong-password").pipe(Effect.flip))._tag).toBe(
          "InvalidCredentials",
        );
      }
      const locked = yield* signIn(ADMIN, ADMIN_PASSWORD).pipe(Effect.flip);
      expect(locked._tag).toBe("AccountLocked");
      expect(locked._tag === "AccountLocked" && locked.retryAfter).toBeGreaterThan(800);
      expect((yield* signIn("nobody@missal.test", "anything").pipe(Effect.flip))._tag).toBe(
        "InvalidCredentials",
      );
    }).pipe(Effect.provide(testLayer())),
  );

  it.live("the network throttle refuses sign-ins before checking the password", () =>
    Effect.gen(function* () {
      for (let attempt = 0; attempt < 3; attempt++) {
        yield* signIn(`someone${attempt}@missal.test`, "x").pipe(Effect.flip);
      }
      const throttled = yield* signIn(ADMIN, BOOTSTRAP).pipe(Effect.flip);
      expect(throttled._tag).toBe("AccountLocked");
    }).pipe(Effect.provide(testLayer(3))),
  );

  it.live("the nightly purge removes expired sessions", () =>
    Effect.gen(function* () {
      const admin = yield* adminSession;
      const sql = yield* SqlClient.SqlClient;
      yield* sql`UPDATE sessions SET expires_at = 0`;
      const accounts = yield* Accounts;
      yield* accounts.purgeExpired.pipe(Effect.provide(RuntimeContext.phantom));
      const [{ count }] = yield* sql<{ count: number }>`SELECT COUNT(*) AS count FROM sessions`;
      expect(count).toBe(0);
      const api = yield* client;
      const ended = yield* api.auth
        .refresh({ payload: { refreshToken: admin.refreshToken } })
        .pipe(Effect.flip);
      expect(ended._tag).toBe("SessionExpired");
    }).pipe(Effect.provide(testLayer())),
  );

  it("emails are trimmed and lowercased at the boundary", () => {
    expect(Schema.decodeUnknownSync(Email)("  Reader@Missal.TEST ")).toBe("reader@missal.test");
    expect(() => Schema.decodeUnknownSync(Email)("not an email")).toThrow();
  });

  it("account responses never carry password material", () => {
    const encoded = Schema.encodeUnknownSync(Account)({
      id: "u",
      email: ADMIN,
      name: "",
      role: "admin",
      passwordHash: "secret-hash",
      passwordSalt: "secret-salt",
      passwordIterations: 1,
      mustChangePassword: false,
      disabled: false,
      createdAt: DateTime.makeUnsafe(0),
      updatedAt: DateTime.makeUnsafe(0),
    } as never);
    expect(JSON.stringify(encoded)).not.toMatch(/secret|password(Hash|Salt|Iterations)/);
  });
});

describe("library", () => {
  it.live("publishers publish, update, download and remove templates", () =>
    Effect.gen(function* () {
      const api = yield* client;
      yield* adminSession;
      const published = yield* api.publishing.create({
        payload: { name: "نمونہ", fieldCount: 2, preview: "p", content: new Uint8Array([1, 2, 3]) },
      });
      expect(published.version).toBe(1);
      const catalog = yield* api.catalog.list();
      expect(
        catalog.templates.map((entry) => [entry.name, entry.fieldCount, entry.version]),
      ).toEqual([["نمونہ", 2, 1]]);
      const first = yield* api.catalog.download({ params: { id: published.id } });
      expect(first.headers["x-missal-version"]).toBe(1);
      expect([...first.body]).toEqual([1, 2, 3]);

      const updated = yield* api.publishing.update({
        params: { id: published.id },
        payload: { name: "نمونہ ۲", fieldCount: 3, preview: "", content: new Uint8Array([4, 5]) },
      });
      expect(updated.version).toBe(2);
      const second = yield* api.catalog.download({ params: { id: published.id } });
      expect([second.headers["x-missal-version"], [...second.body]]).toEqual([2, [4, 5]]);

      yield* api.publishing.remove({ params: { id: published.id } });
      expect((yield* api.catalog.list()).templates).toEqual([]);
      const missing = yield* api.catalog
        .download({ params: { id: published.id } })
        .pipe(Effect.flip);
      expect(missing._tag).toBe("TemplateNotFound");
    }).pipe(Effect.provide(testLayer())),
  );
});

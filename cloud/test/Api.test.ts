import { join } from "node:path";
import { describe, expect, it } from "@effect/vitest";
import * as NodeServices from "@effect/platform-node/NodeServices";
import * as SqliteClient from "@effect/sql-sqlite-node/SqliteClient";
import * as SqliteDrizzle from "drizzle-orm/effect-sqlite-node";
import * as Migrations from "@alchemy.run/sql/Migrations";
import { splitSqliteStatements } from "@alchemy.run/sql/SqlFile";
import * as Http from "alchemy/Http";
import * as ConfigProvider from "effect/ConfigProvider";
import * as Context from "effect/Context";
import * as DateTime from "effect/DateTime";
import * as Duration from "effect/Duration";
import * as Effect from "effect/Effect";
import * as HttpApiMiddleware from "effect/http-api/HttpApiMiddleware";
import * as HttpApiTest from "effect/http-api/HttpApiTest";
import * as HttpClientRequest from "effect/http/HttpClientRequest";
import * as Layer from "effect/Layer";
import * as RateLimiter from "effect/persistence/RateLimiter";
import * as Redacted from "effect/Redacted";
import * as Ref from "effect/Ref";
import * as Schema from "effect/Schema";
import * as SqlClient from "effect/sql/SqlClient";
import * as TestClock from "effect/testing/TestClock";
import { Accounts } from "../src/Accounts.ts";
import { Authentication, Api } from "../src/Api.ts";
import { CryptoLive, ServicesLive } from "../src/App.ts";
import { AuthorizationLive } from "../src/Auth.ts";
import { Blobs } from "../src/Blobs.ts";
import { Atomic, Db } from "../src/Database.ts";
import { HandlersLive } from "../src/Handlers.ts";
import { Mailer, Outbox } from "../src/Mailer.ts";
import { Email } from "../src/Models.ts";
import { AuthThrottle, tooManyAttempts, WINDOW } from "../src/Throttle.ts";
import { SigningKey } from "../src/Tokens.ts";

const ADMIN = "admin@example.test";
const READER = "reader@example.test";
const PASSWORD = "first-secret-123";

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
    yield* Migrations.applyMigrations({
      resolved: {
        dir: join(import.meta.dirname, "..", "migrations"),
        table: Migrations.ALCHEMY_DEFAULT_TABLE,
      },
      executor: {
        dialect: "sqlite",
        query: (text, params) =>
          sql.unsafe<Record<string, unknown>>(text, params).pipe(
            Effect.map((rows) => [...rows]),
            Effect.mapError(Migrations.migrationError("query the test database")),
          ),
        batch: (scripts) =>
          sql
            .withTransaction(
              Effect.forEach(scripts.flatMap(splitSqliteStatements), (statement) =>
                sql.unsafe(statement),
              ),
            )
            .pipe(
              Effect.asVoid,
              Effect.mapError(Migrations.migrationError("migrate the test database")),
            ),
      },
    });
  }).pipe(Effect.provide(NodeServices.layer), Effect.orDie),
);

const signingKey = Layer.effect(
  SigningKey,
  Effect.promise(() =>
    crypto.subtle.importKey(
      "raw",
      new Uint8Array(32).fill(0xab),
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["sign", "verify"],
    ),
  ).pipe(Effect.map(Effect.succeed)),
);

const networkThrottle = (limit: number) =>
  Layer.effect(
    AuthThrottle,
    Effect.map(RateLimiter.RateLimiter, (limiter) =>
      AuthThrottle.of({
        check: (client) =>
          limiter.consume({ key: client, limit, window: WINDOW }).pipe(
            Effect.asVoid,
            Effect.catch((error) =>
              error.reason._tag === "RateLimitExceeded"
                ? Effect.fail(tooManyAttempts(error.reason.retryAfter))
                : Effect.die(error),
            ),
          ),
      }),
    ),
  ).pipe(Layer.provide(RateLimiter.layer), Layer.provide(RateLimiter.layerStoreMemory));

const testLayer = (throttleLimit = 1_000, verifyEmails = true) => {
  const database = Layer.mergeAll(
    Atomic.layerTransaction,
    Layer.effect(Db, SqliteDrizzle.makeWithDefaults()),
    migrate,
  ).pipe(Layer.provideMerge(SqliteClient.layer({ filename: ":memory:" })));
  const infrastructure = Layer.mergeAll(
    database,
    CryptoLive,
    networkThrottle(throttleLimit),
    Blobs.layerMemory,
    Mailer.layerMemory,
    signingKey,
  );
  const app = HandlersLive.pipe(
    Layer.provideMerge(AuthorizationLive),
    Layer.provideMerge(ServicesLive),
  );
  return Layer.mergeAll(app, AuthenticationClient).pipe(
    Layer.provideMerge(infrastructure),
    Layer.provideMerge(CurrentToken.layer),
    Layer.provideMerge(Http.Platform),
    Layer.provideMerge(
      ConfigProvider.layer(
        ConfigProvider.fromUnknown({
          VERIFY_EMAILS: String(verifyEmails),
        }),
      ),
    ),
  );
};

const client = HttpApiTest.groups(Api, [
  "auth",
  "account",
  "catalog",
  "publishing",
  "templateAdmin",
  "users",
]);

const useToken = Effect.fn(function* (token: string | undefined) {
  yield* Ref.set(yield* CurrentToken, token);
});

const mailTo = Effect.fn(function* (email: string) {
  const sent = yield* Ref.get(yield* Outbox);
  return sent.filter((message) => message.to === email);
});

const lastCode = Effect.fn(function* (email: string) {
  const mail = (yield* mailTo(email)).at(-1);
  const code = mail?.text.match(/\b(\d{6})\b/)?.[1];
  if (code === undefined) return yield* Effect.die(`no code was mailed to ${email}`);
  return code;
});

const signUp = (email: string, password = PASSWORD) =>
  Effect.flatMap(client, (api) =>
    api.auth.signUp({ payload: { email, name: "", password: Redacted.make(password) } }),
  );

const signIn = (email: string, password = PASSWORD) =>
  Effect.flatMap(client, (api) =>
    api.auth.signIn({ payload: { email, password: Redacted.make(password) } }),
  );

const verify = (email: string, code: string) =>
  Effect.flatMap(client, (api) => api.auth.verifyEmail({ payload: { email, code } }));

const refresh = (refreshToken: string) =>
  Effect.flatMap(client, (api) => api.auth.refresh({ payload: { refreshToken } }));

const register = Effect.fn(function* (email: string, password = PASSWORD) {
  yield* signUp(email, password);
  const session = yield* verify(email, yield* lastCode(email));
  yield* useToken(session.accessToken);
  return session;
});

const registerAdmin = Effect.fn(function* (email = ADMIN) {
  yield* register(email);
  const sql = yield* SqlClient.SqlClient;
  yield* sql`UPDATE users SET role = 'admin' WHERE email = ${email}`;
  const session = yield* signIn(email);
  yield* useToken(session.accessToken);
  return session;
});

describe("accounts", () => {
  it.effect("signing up mails a code, and verifying signs in with a seven-day trial", () =>
    Effect.gen(function* () {
      const api = yield* client;
      yield* signUp(READER);
      expect(yield* mailTo(READER)).toHaveLength(1);

      const unverified = yield* signIn(READER).pipe(Effect.flip);
      expect(unverified._tag).toBe("EmailNotVerified");
      expect(yield* mailTo(READER)).toHaveLength(1);

      const code = yield* lastCode(READER);
      const wrong = String((Number(code) + 1) % 1_000_000).padStart(6, "0");
      expect((yield* verify(READER, wrong).pipe(Effect.flip))._tag).toBe("InvalidCode");

      const session = yield* verify(READER, code);
      expect(session.account.email).toBe(READER);
      expect(session.account.role).toBe("member");
      expect(session.account.licence.status).toBe("trial");
      expect(DateTime.toEpochMillis(session.account.licence.until)).toBe(
        Duration.toMillis(Duration.days(7)),
      );
      expect((yield* verify(READER, code).pipe(Effect.flip))._tag).toBe("InvalidCode");

      yield* useToken(session.accessToken);
      expect((yield* api.catalog.list()).publisher).toBe(false);
      expect((yield* signIn(READER)).account.licence.status).toBe("trial");
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("with verification off, signing up creates the account and sends no email", () =>
    Effect.gen(function* () {
      yield* signUp(READER);
      expect(yield* mailTo(READER)).toHaveLength(0);
      const session = yield* signIn(READER);
      expect(session.account.email).toBe(READER);
      expect(session.account.licence.status).toBe("trial");
      expect(DateTime.toEpochMillis(session.account.licence.until)).toBe(
        Duration.toMillis(Duration.days(7)),
      );

      yield* signUp(READER, "someone-else-123");
      expect(yield* mailTo(READER)).toHaveLength(0);
      expect((yield* signIn(READER, "someone-else-123").pipe(Effect.flip))._tag).toBe(
        "InvalidCredentials",
      );
      yield* signIn(READER);
    }).pipe(Effect.provide(testLayer(1_000, false))),
  );

  it.effect("sign-up and code requests never reveal whether an email has an account", () =>
    Effect.gen(function* () {
      const api = yield* client;
      yield* register(READER);
      yield* signUp(READER, "someone-else-123");
      const notice = (yield* mailTo(READER)).at(-1);
      expect(notice?.text).toMatch(/already has one/);
      expect(notice?.text).not.toMatch(/\d{6}/);
      const notices = (yield* mailTo(READER)).length;
      yield* signUp(READER, "someone-else-123");
      expect(yield* mailTo(READER)).toHaveLength(notices);
      expect((yield* signIn(READER, "someone-else-123").pipe(Effect.flip))._tag).toBe(
        "InvalidCredentials",
      );
      yield* signIn(READER);

      const before = (yield* mailTo(READER)).length;
      yield* api.auth.requestCode({ payload: { email: READER, purpose: "verify" } });
      expect(yield* mailTo(READER)).toHaveLength(before);
      yield* api.auth.requestCode({ payload: { email: "nobody@example.test", purpose: "reset" } });
      expect(yield* mailTo("nobody@example.test")).toHaveLength(0);
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("five wrong codes discard the code, and a new one waits out the cooldown", () =>
    Effect.gen(function* () {
      const api = yield* client;
      yield* signUp(READER);
      const code = yield* lastCode(READER);
      const wrong = String((Number(code) + 1) % 1_000_000).padStart(6, "0");
      for (let attempt = 0; attempt < 5; attempt++) {
        expect((yield* verify(READER, wrong).pipe(Effect.flip))._tag).toBe("InvalidCode");
      }
      expect((yield* verify(READER, code).pipe(Effect.flip))._tag).toBe("InvalidCode");

      yield* signUp(READER);
      yield* api.auth.requestCode({ payload: { email: READER, purpose: "verify" } });
      expect(yield* mailTo(READER)).toHaveLength(2);
      yield* TestClock.adjust(Duration.seconds(30));
      yield* api.auth.requestCode({ payload: { email: READER, purpose: "verify" } });
      expect(yield* mailTo(READER)).toHaveLength(2);
      yield* TestClock.adjust(Duration.seconds(31));
      yield* api.auth.requestCode({ payload: { email: READER, purpose: "verify" } });
      expect(yield* mailTo(READER)).toHaveLength(3);

      yield* TestClock.adjust(Duration.minutes(11));
      expect((yield* verify(READER, yield* lastCode(READER)).pipe(Effect.flip))._tag).toBe(
        "InvalidCode",
      );
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("a password reset signs in and ends every other session", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const first = yield* register(READER);
      const second = yield* signIn(READER);
      yield* api.auth.requestCode({ payload: { email: READER, purpose: "reset" } });
      const code = yield* lastCode(READER);
      const reset = yield* api.auth.resetPassword({
        payload: { email: READER, code, password: Redacted.make("second-secret-456") },
      });
      expect(reset.account.email).toBe(READER);
      expect((yield* refresh(first.refreshToken).pipe(Effect.flip))._tag).toBe("SessionExpired");
      expect((yield* refresh(second.refreshToken).pipe(Effect.flip))._tag).toBe("SessionExpired");
      yield* refresh(reset.refreshToken);
      expect((yield* signIn(READER).pipe(Effect.flip))._tag).toBe("InvalidCredentials");
      yield* signIn(READER, "second-secret-456");
      const reused = yield* api.auth
        .resetPassword({
          payload: { email: READER, code, password: Redacted.make("third-1234567") },
        })
        .pipe(Effect.flip);
      expect(reused._tag).toBe("InvalidCode");
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("an ended trial blocks the library but not refresh, and a grant reopens it", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const admin = yield* registerAdmin();
      expect(admin.account.licence.status).toBe("licensed");
      const reader = yield* register(READER);

      yield* TestClock.adjust(Duration.days(8));
      const ended = yield* refresh(reader.refreshToken);
      expect(ended.account.licence).toEqual({
        status: "trialEnded",
        until: DateTime.makeUnsafe(Duration.toMillis(Duration.days(7))),
      });
      yield* useToken(ended.accessToken);
      expect((yield* api.catalog.list().pipe(Effect.flip))._tag).toBe("LicenceRequired");
      expect((yield* api.account.me()).licence.status).toBe("trialEnded");

      const adminNow = yield* refresh(admin.refreshToken);
      yield* useToken(adminNow.accessToken);
      expect((yield* api.catalog.list()).publisher).toBe(true);
      const now = yield* DateTime.now;
      const granted = yield* api.users.grantLicence({
        params: { id: reader.account.id },
        payload: { months: 12 },
      });
      expect(granted.licence).toEqual({
        status: "licensed",
        until: DateTime.add(now, { years: 1 }),
      });
      const again = yield* api.users.grantLicence({
        params: { id: reader.account.id },
        payload: { months: 12 },
      });
      expect(again.licence.until).toEqual(DateTime.add(now, { years: 2 }));
      const listed = (yield* api.users.list()).find((account) => account.email === READER);
      expect(listed?.licence.status).toBe("licensed");

      const renewed = yield* refresh(ended.refreshToken);
      expect(renewed.account.licence.status).toBe("licensed");
      yield* useToken(renewed.accessToken);
      expect((yield* api.catalog.list()).templates).toEqual([]);
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("roles gate the API and administrators manage people", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const admin = yield* registerAdmin();
      const own = yield* api.users
        .update({ params: { id: admin.account.id }, payload: { role: "member" } })
        .pipe(Effect.flip);
      expect(own._tag).toBe("OwnAccess");

      const reader = yield* register(READER);
      yield* signUp("pending@example.test");
      expect((yield* api.users.list().pipe(Effect.flip))._tag).toBe("NotAdmin");
      const upload = { name: "x", fieldCount: 0, preview: "", content: new Uint8Array([1, 2, 3]) };
      expect((yield* api.publishing.create({ payload: upload }).pipe(Effect.flip))._tag).toBe(
        "NotPublisher",
      );

      yield* useToken(admin.accessToken);
      const listed = yield* api.users.list();
      expect(listed.map((account) => [account.email, account.emailVerified])).toEqual([
        [ADMIN, true],
        ["pending@example.test", false],
        [READER, true],
      ]);
      yield* api.users.update({
        params: { id: reader.account.id },
        payload: { role: "publisher" },
      });
      const promoted = yield* signIn(READER);
      expect(promoted.account.role).toBe("publisher");

      yield* api.users.update({ params: { id: reader.account.id }, payload: { disabled: true } });
      expect((yield* refresh(promoted.refreshToken).pipe(Effect.flip))._tag).toBe("SessionExpired");
      expect((yield* signIn(READER).pipe(Effect.flip))._tag).toBe("InvalidCredentials");
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("administrators create, verify, renew and remove accounts", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const admin = yield* registerAdmin();
      const now = yield* DateTime.now;
      const created = yield* api.users.create({
        payload: {
          email: READER,
          name: "Reader",
          role: "member",
          password: Redacted.make(PASSWORD),
          months: 6,
        },
      });
      expect(created.emailVerified).toBe(true);
      expect(created.licence).toEqual({
        status: "licensed",
        until: DateTime.add(now, { months: 6 }),
      });
      const taken = yield* api.users
        .create({
          payload: {
            email: READER,
            name: "",
            role: "member",
            password: Redacted.make(PASSWORD),
            months: 0,
          },
        })
        .pipe(Effect.flip);
      expect(taken._tag).toBe("EmailTaken");

      const reader = yield* signIn(READER);
      yield* useToken(admin.accessToken);
      expect((yield* api.users.list()).find((user) => user.id === created.id)?.sessions).toBe(1);

      const ended = yield* api.users.endLicence({ params: { id: created.id } });
      expect(ended.licence.status).toBe("expired");
      const trial = yield* api.users.extendTrial({
        params: { id: created.id },
        payload: { days: 14 },
      });
      expect(trial.licence).toEqual({ status: "trial", until: DateTime.add(now, { days: 14 }) });

      const reset = yield* api.users.setPassword({
        params: { id: created.id },
        payload: { password: Redacted.make("second-secret-456") },
      });
      expect(reset.sessions).toBe(0);
      expect((yield* refresh(reader.refreshToken).pipe(Effect.flip))._tag).toBe("SessionExpired");
      yield* signIn(READER, "second-secret-456");

      yield* useToken(undefined);
      yield* signUp("pending@example.test");
      yield* useToken(admin.accessToken);
      const pending = (yield* api.users.list()).find(
        (user) => user.email === "pending@example.test",
      );
      expect(pending?.emailVerified).toBe(false);
      const confirmed = yield* api.users.verifyEmail({ params: { id: pending?.id ?? "" } });
      expect(confirmed.emailVerified).toBe(true);
      expect(confirmed.licence.status).toBe("trial");
      yield* signIn("pending@example.test");

      yield* useToken(admin.accessToken);
      const self = yield* api.users.remove({ params: { id: admin.account.id } }).pipe(Effect.flip);
      expect(self._tag).toBe("OwnAccess");
      yield* api.users.remove({ params: { id: created.id } });
      expect((yield* signIn(READER, "second-secret-456").pipe(Effect.flip))._tag).toBe(
        "InvalidCredentials",
      );
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("a demoted administrator loses access before the access token expires", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const admin = yield* registerAdmin();
      const sql = yield* SqlClient.SqlClient;
      yield* sql`UPDATE users SET role = 'member' WHERE email = ${ADMIN}`;
      yield* useToken(admin.accessToken);
      expect((yield* api.users.list().pipe(Effect.flip))._tag).toBe("NotAdmin");
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("refresh tokens rotate, a reused one ends the session, and sign-out ends it too", () =>
    Effect.gen(function* () {
      const api = yield* client;
      const session = yield* register(READER);
      const rotated = yield* refresh(session.refreshToken);
      expect(rotated.refreshToken).not.toBe(session.refreshToken);
      expect((yield* refresh(session.refreshToken).pipe(Effect.flip))._tag).toBe("SessionExpired");
      expect((yield* refresh(rotated.refreshToken).pipe(Effect.flip))._tag).toBe("SessionExpired");

      const fresh = yield* signIn(READER);
      yield* api.auth.signOut({ payload: { refreshToken: fresh.refreshToken } });
      expect((yield* refresh(fresh.refreshToken).pipe(Effect.flip))._tag).toBe("SessionExpired");

      yield* useToken(`${fresh.accessToken.slice(0, -2)}xx`);
      expect((yield* api.account.me().pipe(Effect.flip))._tag).toBe("Unauthorized");
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("five wrong passwords lock the account", () =>
    Effect.gen(function* () {
      yield* register(READER);
      for (let attempt = 0; attempt < 5; attempt++) {
        expect((yield* signIn(READER, "wrong-password").pipe(Effect.flip))._tag).toBe(
          "InvalidCredentials",
        );
      }
      const locked = yield* signIn(READER).pipe(Effect.flip);
      expect(locked._tag).toBe("AccountLocked");
      expect(locked._tag === "AccountLocked" && locked.retryAfter).toBeGreaterThan(800);
      expect((yield* signIn("nobody@example.test", "anything").pipe(Effect.flip))._tag).toBe(
        "InvalidCredentials",
      );
    }).pipe(Effect.provide(testLayer())),
  );

  it.effect("the network throttle covers every public auth route", () =>
    Effect.gen(function* () {
      yield* signUp(READER);
      yield* signIn(READER).pipe(Effect.flip);
      yield* verify(READER, "000000").pipe(Effect.flip);
      expect((yield* signUp("someone@example.test").pipe(Effect.flip))._tag).toBe("AccountLocked");
      expect(yield* mailTo("someone@example.test")).toHaveLength(0);
    }).pipe(Effect.provide(testLayer(3))),
  );

  it.effect("the nightly purge removes expired sessions, codes and stale sign-ups", () =>
    Effect.gen(function* () {
      const session = yield* register(READER);
      yield* signUp("pending@example.test");
      const sql = yield* SqlClient.SqlClient;
      yield* sql`UPDATE sessions SET expires_at = 0`;
      yield* TestClock.adjust(Duration.days(8));
      const accounts = yield* Accounts;
      yield* accounts.purgeExpired;
      const counts = yield* sql<{ sessions: number; codes: number; users: number }>`SELECT
        (SELECT COUNT(*) FROM sessions) AS sessions,
        (SELECT COUNT(*) FROM email_codes) AS codes,
        (SELECT COUNT(*) FROM users) AS users`;
      expect(counts[0]).toEqual({ sessions: 0, codes: 0, users: 1 });
      expect((yield* refresh(session.refreshToken).pipe(Effect.flip))._tag).toBe("SessionExpired");
    }).pipe(Effect.provide(testLayer())),
  );

  it("emails are trimmed and lowercased at the boundary", () => {
    expect(Schema.decodeUnknownSync(Email)("  Reader@Example.TEST ")).toBe(READER);
    expect(() => Schema.decodeUnknownSync(Email)("not an email")).toThrow();
  });
});

describe("library", () => {
  it.effect("publishers publish, update, download and remove templates", () =>
    Effect.gen(function* () {
      const api = yield* client;
      yield* registerAdmin();
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

  it.effect("administrators see version history and restore an earlier version", () =>
    Effect.gen(function* () {
      const api = yield* client;
      yield* registerAdmin();
      const { id } = yield* api.publishing.create({
        payload: { name: "پہلا", fieldCount: 1, preview: "a", content: new Uint8Array([1]) },
      });
      yield* api.publishing.update({
        params: { id },
        payload: { name: "دوسرا", fieldCount: 2, preview: "b", content: new Uint8Array([2, 2]) },
      });

      const restored = yield* api.templateAdmin.restore({
        params: { id },
        payload: { version: 1 },
      });
      expect(restored.version).toBe(3);
      const download = yield* api.catalog.download({ params: { id } });
      expect([download.headers["x-missal-version"], [...download.body]]).toEqual([3, [1]]);

      const history = yield* api.templateAdmin.versions({ params: { id } });
      expect(history.map((row) => [row.version, row.name, row.size, row.restoredFrom])).toEqual([
        [3, "پہلا", 1, 1],
        [2, "دوسرا", 2, null],
        [1, "پہلا", 1, null],
      ]);
      expect(history[0]?.publisherEmail).toBe(ADMIN);

      const [managed] = yield* api.templateAdmin.list();
      expect([managed?.version, managed?.versionCount, managed?.publisherEmail]).toEqual([
        3,
        3,
        ADMIN,
      ]);

      const again = yield* api.templateAdmin.restore({ params: { id }, payload: { version: 3 } });
      expect(again.version).toBe(3);
      const absent = yield* api.templateAdmin
        .restore({ params: { id }, payload: { version: 9 } })
        .pipe(Effect.flip);
      expect(absent._tag).toBe("TemplateNotFound");

      yield* register(READER);
      const denied = yield* api.templateAdmin.list().pipe(Effect.flip);
      expect(denied._tag).toBe("NotAdmin");
    }).pipe(Effect.provide(testLayer())),
  );
});

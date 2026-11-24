import * as Alchemy from "alchemy";
import * as Http from "alchemy/Http";
import * as Cloudflare from "@alchemy.run/cloudflare";
import * as SQL from "@alchemy.run/cloudflare/D1/SQL";
import * as Config from "effect/Config";
import * as Effect from "effect/Effect";
import * as FetchHttpClient from "effect/http/FetchHttpClient";
import * as HttpRouter from "effect/http/HttpRouter";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Redacted from "effect/Redacted";
import { Accounts } from "./Accounts.ts";
import { ApiLive, CryptoLive, ServicesLive } from "./App.ts";
import { Blobs } from "./Blobs.ts";
import { Atomic, columnNames } from "./Database.ts";
import { Mailer } from "./Mailer.ts";
import { AuthThrottle } from "./Throttle.ts";
import { SECRET_VARIABLE } from "./Tokens.ts";

export const Catalog = Cloudflare.D1.Database("Catalog", {
  migrations: "./migrations",
});

export const Templates = Cloudflare.R2.Bucket("Templates");

export default Cloudflare.Worker(
  "Api",
  {
    main: import.meta.url,
    compatibility: { date: "2026-11-01" },
    env: { [SECRET_VARIABLE]: Alchemy.makeRandom("TokenSecret") },
  },
  Effect.gen(function* () {
    const resendKey = Option.filter(
      yield* Config.option(Config.Redacted("RESEND_API_KEY")),
      (key) => Redacted.value(key).trim() !== "",
    );
    const mailFrom = Option.filter(
      yield* Config.option(Config.String("MAIL_FROM")),
      (from) => from.trim() !== "",
    );
    const database = yield* Cloudflare.D1.QueryDatabase(Catalog);
    const bucket = yield* Cloudflare.R2.ReadWriteBucket(Templates);
    const limiter = yield* Cloudflare.RateLimit("SIGN_IN_LIMIT", {
      namespaceId: 4721,
      simple: { limit: 10, period: 60 },
    });

    const Infrastructure = Layer.mergeAll(
      Atomic.layerD1.pipe(Layer.provideMerge(SQL.D1Layer(database, columnNames))),
      AuthThrottle.layer(limiter),
      Blobs.layerR2(bucket),
      Option.isSome(resendKey) && Option.isSome(mailFrom)
        ? Mailer.layerResend({ apiKey: resendKey.value, from: mailFrom.value }).pipe(
            Layer.provide(FetchHttpClient.layer),
          )
        : Mailer.layerLog,
    ).pipe(Layer.provideMerge(CryptoLive));

    yield* Cloudflare.Workers.cron("17 3 * * *", () =>
      Effect.gen(function* () {
        const accounts = yield* Accounts;
        yield* accounts.purgeExpired;
      }).pipe(
        Effect.provide(ServicesLive.pipe(Layer.provide(Infrastructure))),
        Effect.catchCause((cause) => Effect.logError("nightly cleanup failed", cause)),
      ),
    );

    return {
      fetch: yield* ApiLive.pipe(
        Layer.provide(Infrastructure),
        Layer.provide(Http.Platform),
        HttpRouter.toHttpEffect,
      ),
    };
  }).pipe(
    Effect.provide(
      Layer.mergeAll(
        Cloudflare.D1.QueryDatabaseBinding,
        Cloudflare.R2.ReadWriteBucketBinding,
        Cloudflare.Workers.RateLimitBinding,
        Cloudflare.Workers.CronEventSourceLive,
        Cloudflare.Workers.Telemetry(),
      ),
    ),
  ),
);

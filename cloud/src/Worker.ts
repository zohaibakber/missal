import * as Cloudflare from "@alchemy.run/cloudflare";
import * as SQL from "@alchemy.run/cloudflare/D1/SQL";
import * as Config from "effect/Config";
import * as Digest from "effect/crypto/Digest";
import * as Kdf from "effect/crypto/Kdf";
import * as Mac from "effect/crypto/Mac";
import * as SecureRandom from "effect/crypto/SecureRandom";
import * as Effect from "effect/Effect";
import * as HttpApiBuilder from "effect/http-api/HttpApiBuilder";
import * as Etag from "effect/http/Etag";
import * as HttpPlatform from "effect/http/HttpPlatform";
import * as HttpRouter from "effect/http/HttpRouter";
import * as Layer from "effect/Layer";
import * as Path from "effect/Path";
import { Accounts } from "./Accounts.ts";
import { MissalApi } from "./Api.ts";
import { AuthorizationLive } from "./Auth.ts";
import { HandlersLive } from "./Handlers.ts";
import { Library } from "./Library.ts";
import { Passwords } from "./Passwords.ts";
import { Tokens } from "./Tokens.ts";

export const Catalog = Cloudflare.D1.Database("Catalog", {
  migrations: "./migrations",
});

export const Templates = Cloudflare.R2.Bucket("Templates");

const WorkerPlatform = Layer.succeed(HttpPlatform.HttpPlatform, {
  platform: "web",
  compression: {
    algorithms: new Set<HttpPlatform.CompressionAlgorithm>(),
    compressResponse: (response) => Effect.succeed(response),
  },
  fileResponse: () => Effect.die("file responses are not served"),
  fileWebResponse: () => Effect.die("file responses are not served"),
});

const CryptoLive = Layer.mergeAll(Kdf.layerWebCrypto, Digest.layerWebCrypto, Mac.layerWebCrypto).pipe(
  Layer.provideMerge(SecureRandom.layerWebCrypto),
);

export default Cloudflare.Worker(
  "Api",
  {
    main: import.meta.url,
    compatibility: { date: "2026-11-01" },
  },
  Effect.gen(function* () {
    const tokenSecret = yield* Config.Redacted("MISSAL_TOKEN_SECRET");
    const adminEmail = yield* Config.String("MISSAL_ADMIN_EMAIL");
    const adminPassword = yield* Config.Redacted("MISSAL_ADMIN_PASSWORD");
    const database = yield* Cloudflare.D1.QueryDatabase(Catalog);
    const bucket = yield* Cloudflare.R2.ReadWriteBucket(Templates);

    const ServicesLive = Layer.mergeAll(
      Accounts.layer({ email: adminEmail, password: adminPassword }),
      Library.layer(bucket),
    ).pipe(
      Layer.provideMerge(Passwords.layer),
      Layer.provideMerge(Tokens.layer(tokenSecret)),
      Layer.provide(SQL.D1Layer(database)),
      Layer.provideMerge(CryptoLive),
    );

    return {
      fetch: HttpApiBuilder.layer(MissalApi).pipe(
        Layer.provide(HandlersLive),
        Layer.provide(AuthorizationLive),
        Layer.provide(ServicesLive),
        Layer.provide([Etag.layer, WorkerPlatform, Path.layer]),
        HttpRouter.toHttpEffect,
      ),
    };
  }).pipe(
    Effect.provide(
      Layer.mergeAll(Cloudflare.D1.QueryDatabaseBinding, Cloudflare.R2.ReadWriteBucketBinding),
    ),
  ),
);

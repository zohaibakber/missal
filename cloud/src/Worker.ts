import * as Cloudflare from "@alchemy.run/cloudflare";
import * as SQL from "@alchemy.run/cloudflare/D1/SQL";
import * as Config from "effect/Config";
import * as Effect from "effect/Effect";
import * as HttpApiBuilder from "effect/http-api/HttpApiBuilder";
import * as HttpApiSchema from "effect/http-api/HttpApiSchema";
import * as Etag from "effect/http/Etag";
import * as HttpPlatform from "effect/http/HttpPlatform";
import * as HttpRouter from "effect/http/HttpRouter";
import * as Layer from "effect/Layer";
import * as Path from "effect/Path";
import { LibraryApi, Viewer } from "./Api.ts";
import { AuthenticationLive, PublishersOnlyLive, parsePublishers } from "./Auth.ts";
import { Library, LibraryLive } from "./Library.ts";

export const Catalog = Cloudflare.D1.Database("Catalog", {
  migrations: "./migrations",
});

export const Templates = Cloudflare.R2.Bucket("Templates");

const CatalogHandlers = HttpApiBuilder.group(LibraryApi, "catalog", (handlers) =>
  Effect.gen(function* () {
    const library = yield* Library;
    return handlers
      .handle("list", () =>
        Effect.gen(function* () {
          const who = yield* Viewer;
          const templates = yield* library.list;
          return { publisher: who.publisher, templates };
        }),
      )
      .handle("download", ({ params }) =>
        library.download(params.id).pipe(
          Effect.map((stored) =>
            HttpApiSchema.withHeaders({
              body: stored.content,
              headers: { "x-missal-version": stored.version },
            }),
          ),
        ),
      );
  }),
);

const PublishingHandlers = HttpApiBuilder.group(LibraryApi, "publishing", (handlers) =>
  Effect.gen(function* () {
    const library = yield* Library;
    return handlers
      .handle("create", ({ payload }) =>
        Effect.flatMap(Viewer, (who) => library.create(payload, who.id)),
      )
      .handle("update", ({ params, payload }) =>
        Effect.flatMap(Viewer, (who) => library.update(params.id, payload, who.id)),
      )
      .handle("remove", ({ params }) => library.remove(params.id));
  }),
);

const HealthHandlers = HttpApiBuilder.group(LibraryApi, "health", (handlers) =>
  handlers.handle("health", () => Effect.succeed("ok")),
);

const WorkerPlatform = Layer.succeed(HttpPlatform.HttpPlatform, {
  platform: "web",
  compression: {
    algorithms: new Set<HttpPlatform.CompressionAlgorithm>(),
    compressResponse: (response) => Effect.succeed(response),
  },
  fileResponse: () => Effect.die("file responses are not served"),
  fileWebResponse: () => Effect.die("file responses are not served"),
});

export default Cloudflare.Worker(
  "Api",
  {
    main: import.meta.url,
    compatibility: { date: "2026-11-01" },
  },
  Effect.gen(function* () {
    const clientId = yield* Config.String("WORKOS_CLIENT_ID");
    const baseUrl = yield* Config.String("WORKOS_BASE_URL").pipe(
      Config.withDefault("https://api.workos.com"),
    );
    const publishers = yield* Config.String("MISSAL_PUBLISHERS");
    const database = yield* Cloudflare.D1.QueryDatabase(Catalog);
    const sql = yield* SQL.D1(database);
    const bucket = yield* Cloudflare.R2.ReadWriteBucket(Templates);

    return {
      fetch: HttpApiBuilder.layer(LibraryApi).pipe(
        Layer.provide([CatalogHandlers, PublishingHandlers, HealthHandlers]),
        Layer.provide(LibraryLive(sql, bucket)),
        Layer.provide(
          AuthenticationLive({ baseUrl, clientId, publishers: parsePublishers(publishers) }),
        ),
        Layer.provide(PublishersOnlyLive),
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

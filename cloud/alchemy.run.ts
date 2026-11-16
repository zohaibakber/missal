import * as Alchemy from "alchemy";
import * as Cloudflare from "@alchemy.run/cloudflare";
import * as Config from "effect/Config";
import * as Effect from "effect/Effect";

export const Catalog = Cloudflare.D1.Database("Catalog", {
  migrations: "./migrations",
});

export const Templates = Cloudflare.R2.Bucket("Templates");

export const Api = Cloudflare.Worker("Api", {
  main: "./src/worker.ts",
  compatibility: { date: "2026-11-01" },
  env: {
    CATALOG: Catalog,
    TEMPLATES: Templates,
    WORKOS_CLIENT_ID: Config.String("WORKOS_CLIENT_ID"),
    PUBLISHERS: Config.String("MISSAL_PUBLISHERS"),
  },
});

export type ApiEnv = Cloudflare.InferEnv<typeof Api>;

export default Alchemy.Stack(
  "MissalLibrary",
  {
    providers: Cloudflare.providers(),
    state: Cloudflare.state(),
  },
  Effect.gen(function* () {
    const api = yield* Api;
    return { url: api.url };
  }),
);

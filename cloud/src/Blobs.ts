import type { ReadWriteBucketClient } from "@alchemy.run/cloudflare/R2/ReadWriteBucket";
import { RuntimeContext } from "alchemy/RuntimeContext";
import * as Context from "effect/Context";
import * as Data from "effect/Data";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";

export class BlobError extends Data.TaggedError("BlobError")<{ readonly cause: unknown }> {}

const perEvent = <A, E>(effect: Effect.Effect<A, E, RuntimeContext>) =>
  Effect.provide(effect, RuntimeContext.phantom);

export class Blobs extends Context.Service<
  Blobs,
  {
    readonly put: (key: string, bytes: Uint8Array) => Effect.Effect<void, BlobError>;
    readonly get: (key: string) => Effect.Effect<Option.Option<Uint8Array>, BlobError>;
    readonly removePrefix: (prefix: string) => Effect.Effect<void, BlobError>;
    readonly remove: (key: string) => Effect.Effect<void, BlobError>;
  }
>()("Blobs") {
  static readonly layerR2 = (bucket: ReadWriteBucketClient) =>
    Layer.succeed(
      Blobs,
      Blobs.of({
        put: (key, bytes) =>
          bucket.put(key, bytes).pipe(
            perEvent,
            Effect.asVoid,
            Effect.mapError((cause) => new BlobError({ cause })),
          ),
        get: (key) =>
          bucket.get(key).pipe(
            perEvent,
            Effect.flatMap((object) =>
              object === null ? Effect.succeedNone : Effect.map(object.bytes(), Option.some),
            ),
            Effect.mapError((cause) => new BlobError({ cause })),
          ),
        remove: (key) =>
          bucket.delete(key).pipe(
            perEvent,
            Effect.mapError((cause) => new BlobError({ cause })),
          ),
        removePrefix: (prefix) =>
          bucket.list({ prefix }).pipe(
            Effect.flatMap(({ objects }) =>
              objects.length === 0
                ? Effect.void
                : bucket.delete(objects.map((object) => object.key)),
            ),
            perEvent,
            Effect.mapError((cause) => new BlobError({ cause })),
          ),
      }),
    );

  static readonly layerMemory = Layer.sync(Blobs, () => {
    const objects = new Map<string, Uint8Array>();
    return Blobs.of({
      put: (key, bytes) => Effect.sync(() => void objects.set(key, bytes)),
      get: (key) => Effect.sync(() => Option.fromNullishOr(objects.get(key))),
      remove: (key) => Effect.sync(() => void objects.delete(key)),
      removePrefix: (prefix) =>
        Effect.sync(() => {
          for (const key of objects.keys()) if (key.startsWith(prefix)) objects.delete(key);
        }),
    });
  });
}

import * as Effect from "effect/Effect";
import { StorageFailure } from "./Api.ts";

export const storage =
  (operation: string) =>
  <A, E, R>(effect: Effect.Effect<A, E, R>): Effect.Effect<A, StorageFailure, R> =>
    effect.pipe(
      Effect.tapError((cause) => Effect.logError(`${operation} failed`, cause)),
      Effect.mapError(() => new StorageFailure({ error: `could not ${operation}` })),
    );

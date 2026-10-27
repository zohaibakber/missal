import * as NodeFileSystem from "@effect/platform-node/NodeFileSystem";
import * as NodePath from "@effect/platform-node/NodePath";
import { Context, Effect, Layer, Option, Schema, SynchronizedRef } from "effect";
import { KeyValueStore } from "effect/persistence";
import type { DesktopPreferences } from "#/desktop-window";

export const DesktopTheme = Schema.Literals(["dark", "light", "system"]);

const Preferences = Schema.Struct({
  theme: DesktopTheme,
  layouts: Schema.Record(Schema.String, Schema.String),
});

const PREFERENCES_FILE = "preferences.json";

const defaults: DesktopPreferences = { theme: "system", layouts: {} };

export class PreferencesStore extends Context.Service<
  PreferencesStore,
  {
    readonly current: () => DesktopPreferences;
    readonly update: (
      f: (preferences: DesktopPreferences) => DesktopPreferences,
    ) => Effect.Effect<void>;
  }
>()("missal/PreferencesStore") {
  static readonly layer = (directory: string) =>
    Layer.effect(
      PreferencesStore,
      Effect.gen(function* () {
        const store = KeyValueStore.toSchemaStore(yield* KeyValueStore.KeyValueStore, Preferences);
        const stored = yield* store.get(PREFERENCES_FILE).pipe(Effect.orElseSucceed(Option.none));
        const preferences = yield* SynchronizedRef.make(Option.getOrElse(stored, () => defaults));

        return PreferencesStore.of({
          current: () => SynchronizedRef.getUnsafe(preferences),
          update: (f) =>
            SynchronizedRef.updateEffect(preferences, (previous) => {
              const next = f(previous);
              return Effect.as(store.set(PREFERENCES_FILE, next), next);
            }).pipe(
              Effect.catchCause((cause) => Effect.logWarning("Preferences were not saved", cause)),
            ),
        });
      }),
    ).pipe(
      Layer.provide(KeyValueStore.layerFileSystem(directory)),
      Layer.provide(Layer.mergeAll(NodeFileSystem.layer, NodePath.layer)),
      Layer.orDie,
    );
}

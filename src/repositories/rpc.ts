import * as BrowserWorker from "@effect/platform-browser/BrowserWorker";
import { Context, Effect, Layer } from "effect";
import { RpcClient } from "effect/rpc";
import type { RpcClientError } from "effect/rpc/RpcClientError";
import { STORAGE_PORT_CHANNEL } from "#/electron/storage-channel";
import { StorageRpcs } from "#/electron/storage-rpc";
import { type RepositoryError, StorageError } from "#/lib/storage-errors";
import {
  FirDocumentRepository,
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  SettingsRepository,
  TemplateRepository,
} from "#/repositories/index";

export class StorageClient extends Context.Service<
  StorageClient,
  RpcClient.FromGroup<typeof StorageRpcs, RpcClientError>
>()("missal/StorageClient") {}

const storagePort = Effect.callback<MessagePort>((resume) => {
  const onMessage = (event: MessageEvent) => {
    const [port] = event.ports;
    if (event.source !== window || event.data !== STORAGE_PORT_CHANNEL || !port) return;
    resume(Effect.succeed(port));
  };
  window.addEventListener("message", onMessage);
  window.electronStorage.connect();
  return Effect.sync(() => window.removeEventListener("message", onMessage));
});

const StorageClientLive = Layer.effect(StorageClient, RpcClient.make(StorageRpcs)).pipe(
  Layer.provide(
    Layer.unwrap(
      Effect.map(storagePort, (port) =>
        RpcClient.layerProtocolWorker({ size: 1 }).pipe(
          Layer.provide(BrowserWorker.layer(() => port)),
        ),
      ),
    ),
  ),
  Layer.orDie,
);

const request = <A>(effect: Effect.Effect<A, RepositoryError | RpcClientError>) =>
  Effect.catchTag(effect, "RpcClientError", () =>
    Effect.fail(
      new StorageError({ message: "Storage request failed", operation: "storage.request" }),
    ),
  );

const call =
  <P extends ReadonlyArray<unknown>, A>(
    procedure: (...payload: P) => Effect.Effect<A, RepositoryError | RpcClientError>,
  ) =>
  (...payload: P) =>
    request(procedure(...payload));

const RepositoriesLive = Layer.effectContext(
  Effect.gen(function* () {
    const client = yield* StorageClient;

    return Context.make(
      PlaceholderRepository,
      PlaceholderRepository.of({
        listGlobals: call(client["Placeholder.listGlobals"])(),
        saveGlobals: call(client["Placeholder.saveGlobals"]),
        list: call(client["Placeholder.list"])(),
        create: call(client["Placeholder.create"]),
        update: call(client["Placeholder.update"]),
        remove: call(client["Placeholder.remove"]),
      }),
    ).pipe(
      Context.add(
        TemplateRepository,
        TemplateRepository.of({
          list: call(client["Template.list"])(),
          search: call(client["Template.search"]),
          get: call(client["Template.get"]),
          create: call(client["Template.create"]),
          save: call(client["Template.save"]),
          remove: call(client["Template.remove"]),
        }),
      ),
      Context.add(
        FirRepository,
        FirRepository.of({
          list: call(client["Fir.list"])(),
          get: call(client["Fir.get"]),
          create: call(client["Fir.create"]),
          update: call(client["Fir.update"]),
          remove: call(client["Fir.remove"]),
          getValueContext: call(client["Fir.valueContext"]),
        }),
      ),
      Context.add(
        FirDocumentRepository,
        FirDocumentRepository.of({
          listForFir: call(client["FirDocument.listForFir"]),
          get: call(client["FirDocument.get"]),
          getMany: call(client["FirDocument.getMany"]),
          addTemplates: call(client["FirDocument.addTemplates"]),
          save: call(client["FirDocument.save"]),
          reorder: call(client["FirDocument.reorder"]),
          remove: call(client["FirDocument.remove"]),
        }),
      ),
      Context.add(
        FirPlaceholderValueRepository,
        FirPlaceholderValueRepository.of({
          listForFir: call(client["FirPlaceholderValue.listForFir"]),
          upsert: call(client["FirPlaceholderValue.upsert"]),
          remove: call(client["FirPlaceholderValue.remove"]),
        }),
      ),
      Context.add(
        SettingsRepository,
        SettingsRepository.of({
          get: call(client["Settings.get"])(),
          save: call(client["Settings.save"]),
          saveFieldMarkers: call(client["Settings.saveFieldMarkers"]),
        }),
      ),
    );
  }),
);

export const RendererRepositoriesLive = RepositoriesLive.pipe(Layer.provide(StorageClientLive));

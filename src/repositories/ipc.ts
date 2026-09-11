import { Cause, Context, Effect, Exit, Layer, Option, Schema } from "effect";
import {
  FirCreateRequest,
  FirGetRequest,
  FirListRequest,
  FirListResult,
  FirPlaceholderValueListRequest,
  FirPlaceholderValueListResult,
  FirPlaceholderValueRemoveRequest,
  FirPlaceholderValueUpsertRequest,
  FirRemoveRequest,
  FirUpdateDocumentRequest,
  FirUpdateRequest,
  PlaceholderCreateRequest,
  PlaceholderListRequest,
  PlaceholderListResult,
  PlaceholderRemoveRequest,
  PlaceholderUpdateRequest,
  SettingsGetRequest,
  SettingsSaveRequest,
  StorageRequest,
  StorageResponse,
  TemplateCreateRequest,
  TemplateGetRequest,
  TemplateListRequest,
  TemplateListResult,
  TemplateRemoveRequest,
  TemplateUpdateRequest,
} from "#/electron/storage-contract";
import { FirRecord } from "#/lib/fir";
import { Placeholder } from "#/lib/placeholder";
import { AppSettings } from "#/lib/settings";
import { StorageError, type RepositoryError } from "#/lib/storage-errors";
import { FirPlaceholderValue, TemplateRecord } from "#/lib/templates";
import {
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  SettingsRepository,
  TemplateRepository,
} from "#/repositories/index";

export class ElectronStorage extends Context.Service<
  ElectronStorage,
  {
    readonly request: (payload: unknown) => Promise<unknown>;
  }
>()("missal/ElectronStorage") {}

export const ElectronStorageLive = Layer.sync(ElectronStorage, () =>
  ElectronStorage.of({
    request: (payload) => {
      const api = globalThis.window?.electronStorage;

      if (!api) {
        return Promise.resolve(
          Schema.encodeUnknownSync(StorageResponse)(
            Exit.fail(
              new StorageError({
                message: "Electron storage is unavailable",
                operation: "storage.request",
              }),
            ),
          ),
        );
      }

      return api.request(payload);
    },
  }),
);

function ipcExit<A>(
  storage: ElectronStorage["Service"],
  request: StorageRequest,
  success: Schema.ConstraintDecoder<A>,
  operation: string,
): Effect.Effect<A, RepositoryError> {
  return Effect.gen(function* () {
    const payload = Schema.encodeUnknownSync(StorageRequest)(request);
    const raw = yield* Effect.tryPromise({
      try: () => storage.request(payload),
      catch: () =>
        new StorageError({
          message: "Storage request failed",
          operation,
        }),
    });
    const exit = yield* Schema.decodeUnknownEffect(StorageResponse)(raw).pipe(
      Effect.mapError(
        () =>
          new StorageError({
            message: "Invalid storage response",
            operation,
          }),
      ),
    );

    if (Exit.isFailure(exit)) {
      return yield* Effect.fail(
        Option.getOrElse(
          Cause.findErrorOption(exit.cause),
          () =>
            new StorageError({
              message: "Storage request failed",
              operation,
            }),
        ),
      );
    }

    return yield* Schema.decodeUnknownEffect(success)(exit.value).pipe(
      Effect.mapError(
        () =>
          new StorageError({
            message: "Invalid storage response",
            operation,
          }),
      ),
    );
  });
}

export const IpcPlaceholderRepositoryLive = Layer.effect(
  PlaceholderRepository,
  Effect.gen(function* () {
    const storage = yield* ElectronStorage;

    return PlaceholderRepository.of({
      list: ipcExit(
        storage,
        PlaceholderListRequest.make({}),
        PlaceholderListResult,
        "placeholder.list",
      ),
      create: (input) =>
        ipcExit(
          storage,
          PlaceholderCreateRequest.make({ input }),
          Placeholder,
          "placeholder.create",
        ),
      update: (input) =>
        ipcExit(
          storage,
          PlaceholderUpdateRequest.make({ input }),
          Placeholder,
          "placeholder.update",
        ),
      remove: (id) =>
        ipcExit(
          storage,
          PlaceholderRemoveRequest.make({ id }),
          Schema.Undefined,
          "placeholder.remove",
        ),
    });
  }),
);

export const IpcTemplateRepositoryLive = Layer.effect(
  TemplateRepository,
  Effect.gen(function* () {
    const storage = yield* ElectronStorage;

    return TemplateRepository.of({
      list: ipcExit(storage, TemplateListRequest.make({}), TemplateListResult, "template.list"),
      get: (id) =>
        ipcExit(storage, TemplateGetRequest.make({ id }), TemplateRecord, "template.get"),
      create: (input) =>
        ipcExit(storage, TemplateCreateRequest.make({ input }), TemplateRecord, "template.create"),
      update: (input) =>
        ipcExit(storage, TemplateUpdateRequest.make({ input }), TemplateRecord, "template.update"),
      remove: (id) =>
        ipcExit(storage, TemplateRemoveRequest.make({ id }), Schema.Undefined, "template.remove"),
    });
  }),
);

export const IpcFirRepositoryLive = Layer.effect(
  FirRepository,
  Effect.gen(function* () {
    const storage = yield* ElectronStorage;

    return FirRepository.of({
      list: ipcExit(storage, FirListRequest.make({}), FirListResult, "fir.list"),
      get: (id) => ipcExit(storage, FirGetRequest.make({ id }), FirRecord, "fir.get"),
      create: (input) =>
        ipcExit(storage, FirCreateRequest.make({ input }), FirRecord, "fir.create"),
      update: (input) =>
        ipcExit(storage, FirUpdateRequest.make({ input }), FirRecord, "fir.update"),
      updateDocument: (input) =>
        ipcExit(storage, FirUpdateDocumentRequest.make({ input }), FirRecord, "fir.updateDocument"),
      remove: (id) =>
        ipcExit(storage, FirRemoveRequest.make({ id }), Schema.Undefined, "fir.remove"),
    });
  }),
);

export const IpcFirPlaceholderValueRepositoryLive = Layer.effect(
  FirPlaceholderValueRepository,
  Effect.gen(function* () {
    const storage = yield* ElectronStorage;

    return FirPlaceholderValueRepository.of({
      listForFir: (firId) =>
        ipcExit(
          storage,
          FirPlaceholderValueListRequest.make({ firId }),
          FirPlaceholderValueListResult,
          "firPlaceholderValue.listForFir",
        ),
      upsert: (input) =>
        ipcExit(
          storage,
          FirPlaceholderValueUpsertRequest.make({ input }),
          FirPlaceholderValue,
          "firPlaceholderValue.upsert",
        ),
      remove: (input) =>
        ipcExit(
          storage,
          FirPlaceholderValueRemoveRequest.make({ input }),
          Schema.Undefined,
          "firPlaceholderValue.remove",
        ),
    });
  }),
);

export const IpcSettingsRepositoryLive = Layer.effect(
  SettingsRepository,
  Effect.gen(function* () {
    const storage = yield* ElectronStorage;

    return SettingsRepository.of({
      get: ipcExit(storage, SettingsGetRequest.make({}), AppSettings, "settings.get"),
      save: (sharedPlaceholders) =>
        ipcExit(
          storage,
          SettingsSaveRequest.make({ sharedPlaceholders }),
          AppSettings,
          "settings.save",
        ),
    });
  }),
);

export const RendererRepositoriesLive = Layer.mergeAll(
  IpcPlaceholderRepositoryLive,
  IpcTemplateRepositoryLive,
  IpcFirRepositoryLive,
  IpcFirPlaceholderValueRepositoryLive,
  IpcSettingsRepositoryLive,
).pipe(Layer.provide(ElectronStorageLive));

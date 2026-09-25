import { Context, Effect, Layer, Match, Schema } from "effect";
import { encodeStorageResponse } from "#/electron/storage-contract";
import {
  GlobalPlaceholderListRequest,
  GlobalPlaceholderSaveRequest,
  GlobalPlaceholderListResult,
  FirCreateRequest,
  FirDocumentAddTemplatesRequest,
  FirDocumentGetRequest,
  FirDocumentListRequest,
  FirDocumentListResult,
  FirDocumentRecordResult,
  FirDocumentRemoveRequest,
  FirDocumentReorderRequest,
  FirDocumentSaveAckResult,
  FirDocumentSaveRequest,
  FirGetRequest,
  FirListRequest,
  FirListResult,
  FirPlaceholderValueListRequest,
  FirPlaceholderValueListResult,
  FirPlaceholderValueRemoveRequest,
  FirPlaceholderValueUpsertRequest,
  FirRemoveRequest,
  FirUpdateRequest,
  FirValueContextRequest,
  FirValueContextResult,
  PlaceholderCreateRequest,
  PlaceholderListRequest,
  PlaceholderListResult,
  PlaceholderRemoveRequest,
  PlaceholderUpdateRequest,
  SettingsGetRequest,
  SettingsSaveFieldMarkersRequest,
  SettingsSaveRequest,
  StorageRequest,
  decodeStorageResponse,
  TemplateCreateRequest,
  TemplateGetRequest,
  TemplateListRequest,
  TemplateListResult,
  TemplateRecordResult,
  TemplateRemoveRequest,
  TemplateSaveAckResult,
  TemplateSaveRequest,
  TemplateSearchRequest,
} from "#/electron/storage-contract";
import { FirRecord } from "#/lib/fir";
import { Placeholder } from "#/lib/placeholder";
import { AppSettings } from "#/lib/settings";
import { StorageError } from "#/lib/storage-errors";
import { FirPlaceholderValue, TemplateSummary } from "#/lib/templates";
import {
  FirDocumentRepository,
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

const ElectronStorageLive = Layer.sync(ElectronStorage, () =>
  ElectronStorage.of({
    request: (payload) => {
      const api = globalThis.window?.electronStorage;

      if (!api) {
        return Promise.resolve(
          encodeStorageResponse({
            _tag: "Failure",
            error: new StorageError({
              message: "Electron storage is unavailable",
              operation: "storage.request",
            }),
          }),
        );
      }

      return api.request(payload);
    },
  }),
);

const ipcExit = Effect.fn("ipcExit")(function* <A>(
  storage: ElectronStorage["Service"],
  request: StorageRequest,
  success: Schema.ConstraintDecoder<A>,
  operation: string,
) {
  const payload = Schema.encodeUnknownSync(StorageRequest)(request);
  const raw = yield* Effect.tryPromise({
    try: () => storage.request(payload),
    catch: () =>
      new StorageError({
        message: "Storage request failed",
        operation,
      }),
  });
  const exit = yield* decodeStorageResponse(raw).pipe(
    Effect.mapError(
      () =>
        new StorageError({
          message: "Invalid storage response",
          operation,
        }),
    ),
  );

  return yield* Match.valueTags(exit, {
    Failure: ({ error }) => error,
    Success: ({ value }) =>
      Schema.decodeUnknownEffect(success)(value).pipe(
        Effect.mapError(
          () =>
            new StorageError({
              message: "Invalid storage response",
              operation,
            }),
        ),
      ),
  });
});

const IpcPlaceholderRepositoryLive = Layer.effect(
  PlaceholderRepository,
  Effect.gen(function* () {
    const storage = yield* ElectronStorage;

    return PlaceholderRepository.of({
      listGlobals: ipcExit(
        storage,
        GlobalPlaceholderListRequest.make({}),
        GlobalPlaceholderListResult,
        "placeholder.listGlobals",
      ),
      saveGlobals: (input) =>
        ipcExit(
          storage,
          GlobalPlaceholderSaveRequest.make({ input }),
          GlobalPlaceholderListResult,
          "placeholder.saveGlobals",
        ),
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

const IpcTemplateRepositoryLive = Layer.effect(
  TemplateRepository,
  Effect.gen(function* () {
    const storage = yield* ElectronStorage;

    return TemplateRepository.of({
      list: ipcExit(storage, TemplateListRequest.make({}), TemplateListResult, "template.list"),
      search: (query) =>
        ipcExit(
          storage,
          TemplateSearchRequest.make({ query }),
          TemplateListResult,
          "template.search",
        ),
      get: (id) =>
        ipcExit(storage, TemplateGetRequest.make({ id }), TemplateRecordResult, "template.get"),
      create: (input) =>
        ipcExit(storage, TemplateCreateRequest.make({ input }), TemplateSummary, "template.create"),
      save: (input) =>
        ipcExit(
          storage,
          TemplateSaveRequest.make({ input }),
          TemplateSaveAckResult,
          "template.save",
        ),
      remove: (id) =>
        ipcExit(storage, TemplateRemoveRequest.make({ id }), Schema.Undefined, "template.remove"),
    });
  }),
);

const IpcFirRepositoryLive = Layer.effect(
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
      remove: (id) =>
        ipcExit(storage, FirRemoveRequest.make({ id }), Schema.Undefined, "fir.remove"),
      getValueContext: (id) =>
        ipcExit(
          storage,
          FirValueContextRequest.make({ id }),
          FirValueContextResult,
          "fir.valueContext",
        ),
    });
  }),
);

const IpcFirDocumentRepositoryLive = Layer.effect(
  FirDocumentRepository,
  Effect.gen(function* () {
    const storage = yield* ElectronStorage;

    return FirDocumentRepository.of({
      listForFir: (firId) =>
        ipcExit(
          storage,
          FirDocumentListRequest.make({ firId }),
          FirDocumentListResult,
          "firDocument.listForFir",
        ),
      get: (id) =>
        ipcExit(
          storage,
          FirDocumentGetRequest.make({ id }),
          FirDocumentRecordResult,
          "firDocument.get",
        ),
      addTemplates: (input) =>
        ipcExit(
          storage,
          FirDocumentAddTemplatesRequest.make({ input }),
          FirDocumentListResult,
          "firDocument.addTemplates",
        ),
      save: (input) =>
        ipcExit(
          storage,
          FirDocumentSaveRequest.make({ input }),
          FirDocumentSaveAckResult,
          "firDocument.save",
        ),
      reorder: (input) =>
        ipcExit(
          storage,
          FirDocumentReorderRequest.make({ input }),
          FirDocumentListResult,
          "firDocument.reorder",
        ),
      remove: (id) =>
        ipcExit(
          storage,
          FirDocumentRemoveRequest.make({ id }),
          Schema.Undefined,
          "firDocument.remove",
        ),
    });
  }),
);

const IpcFirPlaceholderValueRepositoryLive = Layer.effect(
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

const IpcSettingsRepositoryLive = Layer.effect(
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
      saveFieldMarkers: (fieldMarkers) =>
        ipcExit(
          storage,
          SettingsSaveFieldMarkersRequest.make({ fieldMarkers }),
          AppSettings,
          "settings.saveFieldMarkers",
        ),
    });
  }),
);

export const RendererRepositoriesLive = Layer.mergeAll(
  IpcPlaceholderRepositoryLive,
  IpcTemplateRepositoryLive,
  IpcFirRepositoryLive,
  IpcFirDocumentRepositoryLive,
  IpcFirPlaceholderValueRepositoryLive,
  IpcSettingsRepositoryLive,
).pipe(Layer.provide(ElectronStorageLive));

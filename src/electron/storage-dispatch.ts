import { Cause, Effect, Layer, Match, Option, Schema } from "effect";
import {
  GlobalPlaceholderListResult,
  FirDocumentListResult,
  FirDocumentRecordListResult,
  FirDocumentRecordResult,
  FirDocumentSaveAckResult,
  FirListResult,
  FirPlaceholderValueListResult,
  FirValueContextResult,
  PlaceholderListResult,
  StorageRequest,
  type StorageResponse,
  decodeStorageRequest,
  encodeStorageResponse,
  TemplateListResult,
  TemplateRecordResult,
  TemplateSaveAckResult,
} from "#/electron/storage-contract";
import { StorageRpcs, type StorageWorkerConfig } from "#/electron/storage-rpc";
import { storageLayer } from "#/electron/storage-runtime";
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

const handleStorageRequest = Effect.fnUntraced(function* (request: StorageRequest) {
  return yield* Match.valueTags(request, {
    "Placeholder.listGlobals": () =>
      Effect.gen(function* () {
        const repository = yield* PlaceholderRepository;
        return Schema.encodeUnknownSync(GlobalPlaceholderListResult)(yield* repository.listGlobals);
      }),
    "Placeholder.saveGlobals": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* PlaceholderRepository;
        return Schema.encodeUnknownSync(GlobalPlaceholderListResult)(
          yield* repository.saveGlobals(input),
        );
      }),
    "Placeholder.list": () =>
      Effect.gen(function* () {
        const repository = yield* PlaceholderRepository;
        return Schema.encodeUnknownSync(PlaceholderListResult)(yield* repository.list);
      }),
    "Placeholder.create": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* PlaceholderRepository;
        return Schema.encodeUnknownSync(Placeholder)(yield* repository.create(input));
      }),
    "Placeholder.update": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* PlaceholderRepository;
        return Schema.encodeUnknownSync(Placeholder)(yield* repository.update(input));
      }),
    "Placeholder.remove": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* PlaceholderRepository;
        yield* repository.remove(id);
        return undefined;
      }),
    "Template.list": () =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        return Schema.encodeUnknownSync(TemplateListResult)(yield* repository.list);
      }),
    "Template.search": ({ query }) =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        return Schema.encodeUnknownSync(TemplateListResult)(yield* repository.search(query));
      }),
    "Template.get": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        return Schema.encodeUnknownSync(TemplateRecordResult)(yield* repository.get(id));
      }),
    "Template.create": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        return Schema.encodeUnknownSync(TemplateSummary)(yield* repository.create(input));
      }),
    "Template.save": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        return Schema.encodeUnknownSync(TemplateSaveAckResult)(yield* repository.save(input));
      }),
    "Template.remove": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        yield* repository.remove(id);
        return undefined;
      }),
    "Fir.list": () =>
      Effect.gen(function* () {
        const repository = yield* FirRepository;
        return Schema.encodeUnknownSync(FirListResult)(yield* repository.list);
      }),
    "Fir.get": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* FirRepository;
        return Schema.encodeUnknownSync(FirRecord)(yield* repository.get(id));
      }),
    "Fir.create": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* FirRepository;
        return Schema.encodeUnknownSync(FirRecord)(yield* repository.create(input));
      }),
    "Fir.update": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* FirRepository;
        return Schema.encodeUnknownSync(FirRecord)(yield* repository.update(input));
      }),
    "Fir.remove": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* FirRepository;
        yield* repository.remove(id);
        return undefined;
      }),
    "Fir.valueContext": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* FirRepository;
        return Schema.encodeUnknownSync(FirValueContextResult)(
          yield* repository.getValueContext(id),
        );
      }),
    "FirDocument.listForFir": ({ firId }) =>
      Effect.gen(function* () {
        const repository = yield* FirDocumentRepository;
        return Schema.encodeUnknownSync(FirDocumentListResult)(yield* repository.listForFir(firId));
      }),
    "FirDocument.get": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* FirDocumentRepository;
        return Schema.encodeUnknownSync(FirDocumentRecordResult)(yield* repository.get(id));
      }),
    "FirDocument.getMany": ({ ids }) =>
      Effect.gen(function* () {
        const repository = yield* FirDocumentRepository;
        return Schema.encodeUnknownSync(FirDocumentRecordListResult)(
          yield* repository.getMany(ids),
        );
      }),
    "FirDocument.addTemplates": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* FirDocumentRepository;
        return Schema.encodeUnknownSync(FirDocumentListResult)(
          yield* repository.addTemplates(input),
        );
      }),
    "FirDocument.save": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* FirDocumentRepository;
        return Schema.encodeUnknownSync(FirDocumentSaveAckResult)(yield* repository.save(input));
      }),
    "FirDocument.reorder": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* FirDocumentRepository;
        return Schema.encodeUnknownSync(FirDocumentListResult)(yield* repository.reorder(input));
      }),
    "FirDocument.remove": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* FirDocumentRepository;
        yield* repository.remove(id);
        return undefined;
      }),
    "FirPlaceholderValue.listForFir": ({ firId }) =>
      Effect.gen(function* () {
        const repository = yield* FirPlaceholderValueRepository;
        return Schema.encodeUnknownSync(FirPlaceholderValueListResult)(
          yield* repository.listForFir(firId),
        );
      }),
    "FirPlaceholderValue.upsert": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* FirPlaceholderValueRepository;
        return Schema.encodeUnknownSync(FirPlaceholderValue)(yield* repository.upsert(input));
      }),
    "FirPlaceholderValue.remove": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* FirPlaceholderValueRepository;
        yield* repository.remove(input);
        return undefined;
      }),
    "Settings.get": () =>
      Effect.gen(function* () {
        const repository = yield* SettingsRepository;
        return Schema.encodeUnknownSync(AppSettings)(yield* repository.get);
      }),
    "Settings.save": ({ sharedPlaceholders }) =>
      Effect.gen(function* () {
        const repository = yield* SettingsRepository;
        return Schema.encodeUnknownSync(AppSettings)(yield* repository.save(sharedPlaceholders));
      }),
    "Settings.saveFieldMarkers": ({ fieldMarkers }) =>
      Effect.gen(function* () {
        const repository = yield* SettingsRepository;
        return Schema.encodeUnknownSync(AppSettings)(
          yield* repository.saveFieldMarkers(fieldMarkers),
        );
      }),
  });
});

function encodeResponse(response: StorageResponse) {
  return encodeStorageResponse(response);
}

const requestFailed = () =>
  new StorageError({
    message: "Storage operation failed",
    operation: "storage.request",
  });

export const dispatchStorageRequest = (payload: string) =>
  Effect.gen(function* () {
    const request = yield* decodeStorageRequest(payload).pipe(
      Effect.mapError(
        () =>
          new StorageError({
            message: "Invalid storage request",
            operation: "storage.decode",
          }),
      ),
    );
    return yield* handleStorageRequest(request);
  }).pipe(
    Effect.match({
      onFailure: (error) => encodeResponse({ _tag: "Failure", error }),
      onSuccess: (value) => encodeResponse({ _tag: "Success", value }),
    }),
    Effect.catchDefect((defect) => {
      console.error("Missal storage defect", defect);
      return Effect.succeed(encodeResponse({ _tag: "Failure", error: requestFailed() }));
    }),
  );

export const storageHandlers = (config: StorageWorkerConfig) =>
  StorageRpcs.toLayer(
    Effect.gen(function* () {
      const database = yield* Effect.exit(Layer.build(storageLayer(config)));

      return StorageRpcs.of({
        "Storage.open": () => Effect.asVoid(database),
        "Storage.request": ({ payload }) =>
          Effect.flatMap(database, (context) =>
            dispatchStorageRequest(payload).pipe(Effect.provide(context)),
          ).pipe(
            // The database never opened; answer with the reason instead of failing the RPC.
            Effect.catchCause((cause) =>
              Effect.succeed(
                encodeResponse({
                  _tag: "Failure",
                  error: Option.getOrElse(
                    Option.filter(Cause.findErrorOption(cause), Schema.is(StorageError)),
                    requestFailed,
                  ),
                }),
              ),
            ),
          ),
      });
    }),
  );

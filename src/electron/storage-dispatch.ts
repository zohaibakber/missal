import { Effect, Exit, Match, Schema } from "effect";
import {
  FirListResult,
  FirPlaceholderValueListResult,
  PlaceholderListResult,
  StorageRequest,
  StorageResponse,
  TemplateListResult,
} from "#/electron/storage-contract";
import { FirRecord } from "#/lib/fir";
import { Placeholder } from "#/lib/placeholder";
import { AppSettings } from "#/lib/settings";
import { RepositoryError, StorageError } from "#/lib/storage-errors";
import { FirPlaceholderValue, TemplateRecord } from "#/lib/templates";
import {
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  SettingsRepository,
  TemplateRepository,
} from "#/repositories/index";

const handleStorageRequest = Effect.fnUntraced(function* (request: StorageRequest) {
  return yield* Match.valueTags(request, {
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
    "Template.get": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        return Schema.encodeUnknownSync(TemplateRecord)(yield* repository.get(id));
      }),
    "Template.create": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        return Schema.encodeUnknownSync(TemplateRecord)(yield* repository.create(input));
      }),
    "Template.update": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* TemplateRepository;
        return Schema.encodeUnknownSync(TemplateRecord)(yield* repository.update(input));
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
    "Fir.updateDocument": ({ input }) =>
      Effect.gen(function* () {
        const repository = yield* FirRepository;
        return Schema.encodeUnknownSync(FirRecord)(yield* repository.updateDocument(input));
      }),
    "Fir.remove": ({ id }) =>
      Effect.gen(function* () {
        const repository = yield* FirRepository;
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
  });
});

function encodeResponse(exit: Exit.Exit<unknown, RepositoryError>) {
  return Schema.encodeUnknownSync(StorageResponse)(exit);
}

export const dispatchStorageRequest = (payload: unknown) =>
  Effect.gen(function* () {
    const request = yield* Schema.decodeUnknownEffect(StorageRequest)(payload).pipe(
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
      onFailure: (error) => encodeResponse(Exit.fail(error)),
      onSuccess: (value) => encodeResponse(Exit.succeed(value)),
    }),
    Effect.catchDefect((defect) => {
      console.error("Missal storage defect", defect);
      return Effect.succeed(
        encodeResponse(
          Exit.fail(
            new StorageError({
              message: "Storage operation failed",
              operation: "storage.request",
            }),
          ),
        ),
      );
    }),
  );

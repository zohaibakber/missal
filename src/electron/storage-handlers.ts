import { Effect, Layer } from "effect";
import type { MessagePort } from "node:worker_threads";
import { RpcServer } from "effect/rpc";
import * as MessagePortRunner from "#/electron/message-port-runner";
import { StorageControlRpcs, type StorageWorkerConfig } from "#/electron/storage-control-rpc";
import { StorageRpcs } from "#/electron/storage-rpc";
import { storageLayer } from "#/electron/storage-runtime";
import { StorageError } from "#/lib/storage-errors";
import {
  FirDocumentRepository,
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  SettingsRepository,
  TemplateRepository,
} from "#/repositories/index";

export const StorageHandlers = StorageRpcs.toLayer(
  Effect.gen(function* () {
    const placeholders = yield* PlaceholderRepository;
    const templates = yield* TemplateRepository;
    const firs = yield* FirRepository;
    const firDocuments = yield* FirDocumentRepository;
    const values = yield* FirPlaceholderValueRepository;
    const settings = yield* SettingsRepository;

    return StorageRpcs.of({
      "Placeholder.listGlobals": () => placeholders.listGlobals,
      "Placeholder.saveGlobals": (input) => placeholders.saveGlobals(input),
      "Placeholder.list": () => placeholders.list,
      "Placeholder.create": (input) => placeholders.create(input),
      "Placeholder.update": (input) => placeholders.update(input),
      "Placeholder.remove": (id) => placeholders.remove(id),
      "Template.list": () => templates.list,
      "Template.search": (query) => templates.search(query),
      "Template.get": (id) => templates.get(id),
      "Template.create": (input) => templates.create(input),
      "Template.save": (input) => templates.save(input),
      "Template.remove": (id) => templates.remove(id),
      "Fir.list": () => firs.list,
      "Fir.get": (id) => firs.get(id),
      "Fir.create": (input) => firs.create(input),
      "Fir.update": (input) => firs.update(input),
      "Fir.remove": (id) => firs.remove(id),
      "Fir.valueContext": (id) => firs.getValueContext(id),
      "FirDocument.listForFir": (firId) => firDocuments.listForFir(firId),
      "FirDocument.get": (id) => firDocuments.get(id),
      "FirDocument.getMany": (ids) => firDocuments.getMany(ids),
      "FirDocument.addTemplates": (input) => firDocuments.addTemplates(input),
      "FirDocument.save": (input) => firDocuments.save(input),
      "FirDocument.reorder": (input) => firDocuments.reorder(input),
      "FirDocument.remove": (id) => firDocuments.remove(id),
      "FirPlaceholderValue.listForFir": (firId) => values.listForFir(firId),
      "FirPlaceholderValue.upsert": (input) => values.upsert(input),
      "FirPlaceholderValue.remove": (input) => values.remove(input),
      "Settings.get": () => settings.get,
      "Settings.save": (sharedPlaceholders) => settings.save(sharedPlaceholders),
      "Settings.saveFieldMarkers": (fieldMarkers) => settings.saveFieldMarkers(fieldMarkers),
    });
  }),
);

const serveRenderer = (port: MessagePort) =>
  RpcServer.layer(StorageRpcs).pipe(
    Layer.provide(StorageHandlers),
    Layer.provide(Layer.fresh(RpcServer.layerProtocolWorkerRunner)),
    Layer.provide(MessagePortRunner.layer(port)),
    Layer.launch,
  );

const openFailed = (cause: unknown) =>
  new StorageError({
    message: cause instanceof Error ? cause.message : "Storage operation failed",
    operation: "database.open",
  });

export const storageControlHandlers = (config: StorageWorkerConfig) =>
  StorageControlRpcs.toLayer(
    Effect.gen(function* () {
      const storage = yield* Effect.exit(
        Layer.build(storageLayer(config)).pipe(
          Effect.mapError((error) => (error instanceof StorageError ? error : openFailed(error))),
          Effect.catchDefect((defect) => Effect.fail(openFailed(defect))),
        ),
      );
      const scope = yield* Effect.scope;

      return StorageControlRpcs.of({
        "Storage.open": () => Effect.asVoid(storage),
        "Storage.connect": ({ port }) =>
          Effect.flatMap(storage, (context) =>
            serveRenderer(port).pipe(Effect.provide(context), Effect.forkIn(scope), Effect.asVoid),
          ),
      });
    }),
  );

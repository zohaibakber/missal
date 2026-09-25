import type { SaveGlobalPlaceholdersInput } from "#/lib/global-placeholder";
import { Effect } from "effect";
import { AsyncResult, Atom, Reactivity } from "effect/unstable/reactivity";
import type { DocumentEnvelope } from "#/lib/document-format";
import type { FirCreateInput, FirId, FirUpdateInput } from "#/lib/fir";
import type { FirDocumentId, TemplateId } from "#/lib/ids";
import {
  indexPlaceholders,
  type PlaceholderCreateInput,
  type PlaceholderUpdateInput,
} from "#/lib/placeholder";
import {
  FirDocumentRecord,
  FirDocumentSaveInput,
  type AddFirTemplatesInput,
  type FirDocumentSummary,
  type ReorderFirDocumentsInput,
} from "#/lib/fir-document";
import { TemplateRecord, TemplateUpdateInput, type TemplateCreateInput } from "#/lib/templates";
import { DEFAULT_FIELD_MARKERS, type FieldMarkers } from "#/lib/settings";
import {
  FirDocumentRepository,
  FirRepository,
  PlaceholderRepository,
  SettingsRepository,
  TemplateRepository,
  type AppRepositories,
} from "#/repositories/index";
import { appRuntime } from "#/state/app-runtime";

const LATEST_LIMIT = 5;
const IDLE_TTL = "1 minute";

/**
 * A read-through cache that a mutation can overwrite with what it just stored, so saving a
 * document doesn't download the same body straight back. Refreshing re-reads the source.
 */
function overridable<A>(source: Atom.Atom<A>) {
  return Atom.writable(
    (get) => get(source),
    (ctx, value: A) => ctx.setSelf(value),
    (refresh) => refresh(source),
  );
}

function reorderedDocuments(
  documents: readonly FirDocumentSummary[],
  documentIds: readonly FirDocumentId[],
) {
  const byId = new Map(documents.map((document) => [document.id, document]));
  return documentIds.flatMap((id) => byId.get(id) ?? []);
}

function makeAppAtoms(runtime: Atom.AtomRuntime<AppRepositories>) {
  const placeholdersAtom = runtime
    .atom(Effect.flatMap(PlaceholderRepository, (repository) => repository.list))
    .pipe(runtime.factory.withReactivity(["placeholders"]), Atom.keepAlive);

  const globalPlaceholdersAtom = runtime
    .atom(Effect.flatMap(PlaceholderRepository, (repository) => repository.listGlobals))
    .pipe(runtime.factory.withReactivity(["placeholders", "settings"]));
  const saveGlobalPlaceholdersAtom = runtime.fn(
    (input: SaveGlobalPlaceholdersInput) =>
      Effect.flatMap(PlaceholderRepository, (repository) => repository.saveGlobals(input)),
    { reactivityKeys: ["placeholders", "settings"] },
  );

  const placeholderIndexAtom = Atom.mapResult(placeholdersAtom, indexPlaceholders);

  const settingsAtom = runtime
    .atom(Effect.flatMap(SettingsRepository, (repository) => repository.get))
    .pipe(runtime.factory.withReactivity(["settings"]), Atom.keepAlive);

  const fieldMarkersAtom = Atom.map(settingsAtom, (result) =>
    AsyncResult.isSuccess(result) ? result.value.fieldMarkers : DEFAULT_FIELD_MARKERS,
  );

  const saveFieldMarkersAtom = runtime.fn(
    (fieldMarkers: FieldMarkers) =>
      Effect.flatMap(SettingsRepository, (repository) => repository.saveFieldMarkers(fieldMarkers)),
    { reactivityKeys: ["settings"] },
  );

  const templatesAtom = runtime
    .atom(Effect.flatMap(TemplateRepository, (repository) => repository.list))
    .pipe(runtime.factory.withReactivity(["templates"]), Atom.keepAlive);

  // The body is keyed apart from the list: saving refreshes the list, not every open body.
  const templateByIdAtom = Atom.family((templateId: TemplateId) =>
    overridable(
      runtime
        .atom(Effect.flatMap(TemplateRepository, (repository) => repository.get(templateId)))
        .pipe(runtime.factory.withReactivity([`template:${templateId}`])),
    ).pipe(Atom.setIdleTTL(IDLE_TTL)),
  );

  const firsAtom = runtime
    .atom(Effect.flatMap(FirRepository, (repository) => repository.list))
    .pipe(runtime.factory.withReactivity(["firs"]), Atom.keepAlive);

  const latestFirsAtom = Atom.mapResult(firsAtom, (firs) =>
    firs.toSorted((first, second) => second.id - first.id).slice(0, LATEST_LIMIT),
  );

  const firByIdAtom = Atom.family((firId: FirId) =>
    runtime
      .atom(Effect.flatMap(FirRepository, (repository) => repository.get(firId)))
      .pipe(runtime.factory.withReactivity(["firs", `fir:${firId}`]), Atom.setIdleTTL(IDLE_TTL)),
  );

  const firValueContextAtom = Atom.family((firId: FirId) =>
    runtime
      .atom(Effect.flatMap(FirRepository, (repository) => repository.getValueContext(firId)))
      .pipe(
        runtime.factory.withReactivity([
          "placeholders",
          "settings",
          "fir-values",
          `fir-values:${firId}`,
          `fir:${firId}`,
        ]),
        Atom.setIdleTTL(IDLE_TTL),
      ),
  );

  // Optimistic, so reordering or removing a document updates the list before storage answers.
  const firDocumentsAtom = Atom.family((firId: FirId) =>
    Atom.optimistic(
      runtime
        .atom(Effect.flatMap(FirDocumentRepository, (repository) => repository.listForFir(firId)))
        .pipe(runtime.factory.withReactivity(["fir-documents", `fir-documents:${firId}`])),
    ).pipe(Atom.setIdleTTL(IDLE_TTL)),
  );

  const firDocumentByIdAtom = Atom.family((documentId: FirDocumentId | null) =>
    overridable(
      runtime
        .atom(
          documentId === null
            ? Effect.succeed(null)
            : Effect.flatMap(FirDocumentRepository, (repository) => repository.get(documentId)),
        )
        .pipe(runtime.factory.withReactivity([`fir-document:${documentId}`])),
    ).pipe(Atom.setIdleTTL(IDLE_TTL)),
  );

  const createPlaceholderAtom = runtime.fn(
    (input: PlaceholderCreateInput) =>
      Effect.flatMap(PlaceholderRepository, (repository) => repository.create(input)),
    { reactivityKeys: ["placeholders"] },
  );

  const updatePlaceholderAtom = runtime.fn(
    (input: PlaceholderUpdateInput) =>
      Effect.flatMap(PlaceholderRepository, (repository) => repository.update(input)),
    { reactivityKeys: ["placeholders"] },
  );

  const createTemplateAtom = runtime.fn(
    (input: TemplateCreateInput) =>
      Effect.flatMap(TemplateRepository, (repository) => repository.create(input)),
    { reactivityKeys: ["templates"] },
  );

  const saveTemplateAtom = runtime.fn(
    Effect.fnUntraced(function* (
      {
        current,
        document,
        name,
      }: { current: TemplateRecord; document: DocumentEnvelope; name: string },
      get: Atom.FnContext,
    ) {
      const repository = yield* TemplateRepository;
      const ack = yield* repository.save(
        new TemplateUpdateInput({
          document,
          expectedRevision: current.revision,
          id: current.id,
          name,
        }),
      );
      get.set(
        templateByIdAtom(current.id),
        AsyncResult.success(
          new TemplateRecord({
            createdAt: current.createdAt,
            document,
            id: current.id,
            name,
            revision: ack.revision,
            updatedAt: ack.updatedAt,
          }),
        ),
      );
      return ack;
    }),
    { reactivityKeys: ["templates"] },
  );

  const removeTemplateAtom = runtime.fn((id: TemplateId) =>
    Effect.flatMap(TemplateRepository, (repository) => repository.remove(id)).pipe(
      Reactivity.mutation(["templates", `template:${id}`]),
    ),
  );

  const createFirAtom = runtime.fn(
    (input: FirCreateInput) =>
      Effect.flatMap(FirRepository, (repository) => repository.create(input)),
    { reactivityKeys: ["firs"] },
  );

  const updateFirAtom = runtime.fn((input: FirUpdateInput) =>
    Effect.flatMap(FirRepository, (repository) => repository.update(input)).pipe(
      Reactivity.mutation(["firs", `fir:${input.id}`]),
    ),
  );

  const removeFirAtom = runtime.fn((id: FirId) =>
    Effect.flatMap(FirRepository, (repository) => repository.remove(id)).pipe(
      Reactivity.mutation(["firs", `fir:${id}`, "fir-values", "fir-documents"]),
    ),
  );

  const addFirTemplatesAtom = runtime.fn((input: AddFirTemplatesInput) =>
    Effect.flatMap(FirDocumentRepository, (repository) => repository.addTemplates(input)).pipe(
      Reactivity.mutation([`fir-documents:${input.firId}`]),
    ),
  );

  const loadPrintDocumentsAtom = runtime.fn((ids: readonly FirDocumentId[]) =>
    Effect.flatMap(FirDocumentRepository, (repository) => repository.getMany(ids)),
  );

  const saveFirDocumentAtom = runtime.fn(
    Effect.fnUntraced(function* (
      { current, document }: { current: FirDocumentRecord; document: DocumentEnvelope },
      get: Atom.FnContext,
    ) {
      const repository = yield* FirDocumentRepository;
      const ack = yield* repository.save(
        new FirDocumentSaveInput({ document, expectedRevision: current.revision, id: current.id }),
      );
      get.set(
        firDocumentByIdAtom(current.id),
        AsyncResult.success(
          new FirDocumentRecord({
            createdAt: current.createdAt,
            document,
            firId: current.firId,
            id: current.id,
            position: current.position,
            revision: ack.revision,
            sourceTemplateRevision: current.sourceTemplateRevision,
            templateId: current.templateId,
            title: current.title,
            updatedAt: ack.updatedAt,
          }),
        ),
      );
      return ack;
    }),
  );

  const reorderFirDocumentsAtom = Atom.family((firId: FirId) =>
    firDocumentsAtom(firId).pipe(
      Atom.optimisticFn({
        reducer: (current, input: ReorderFirDocumentsInput) =>
          AsyncResult.map(current, (documents) => reorderedDocuments(documents, input.documentIds)),
        fn: runtime.fn((input: ReorderFirDocumentsInput) =>
          Effect.flatMap(FirDocumentRepository, (repository) => repository.reorder(input)),
        ),
      }),
    ),
  );

  const removeFirDocumentAtom = Atom.family((firId: FirId) =>
    firDocumentsAtom(firId).pipe(
      Atom.optimisticFn({
        reducer: (current, id: FirDocumentId) =>
          AsyncResult.map(current, (documents) =>
            documents.filter((document) => document.id !== id),
          ),
        fn: runtime.fn((id: FirDocumentId) =>
          Effect.flatMap(FirDocumentRepository, (repository) => repository.remove(id)),
        ),
      }),
    ),
  );

  return {
    globalPlaceholdersAtom,
    saveGlobalPlaceholdersAtom,
    placeholdersAtom,
    placeholderIndexAtom,
    settingsAtom,
    fieldMarkersAtom,
    saveFieldMarkersAtom,
    templatesAtom,
    templateByIdAtom,
    firsAtom,
    latestFirsAtom,
    firByIdAtom,
    firValueContextAtom,
    firDocumentsAtom,
    firDocumentByIdAtom,
    createPlaceholderAtom,
    updatePlaceholderAtom,
    createTemplateAtom,
    saveTemplateAtom,
    removeTemplateAtom,
    createFirAtom,
    updateFirAtom,
    removeFirAtom,
    addFirTemplatesAtom,
    saveFirDocumentAtom,
    loadPrintDocumentsAtom,
    reorderFirDocumentsAtom,
    removeFirDocumentAtom,
  };
}

export const atoms = makeAppAtoms(appRuntime);

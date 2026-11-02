import type { SaveGlobalPlaceholdersInput } from "#/lib/global-placeholder";
import { Effect } from "effect";
import { AsyncResult, Atom, Reactivity } from "effect/reactivity";
import type { FirCreateInput, FirId, FirUpdateInput } from "#/lib/fir";
import type { FirDocumentId, TemplateId } from "#/lib/ids";
import {
  indexFields,
  type PlaceholderCreateInput,
  type PlaceholderUpdateInput,
} from "#/lib/placeholder";
import { catalogFieldPresentation, type FieldDisplayMode } from "#/lib/field";
import {
  type AddFirTemplatesInput,
  type FirDocumentSaveInput,
  type FirDocumentSummary,
  type ReorderFirDocumentsInput,
} from "#/lib/fir-document";
import type { TemplateCreateInput, TemplateUpdateInput } from "#/lib/templates";
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
    .pipe(runtime.factory.withReactivity(["placeholders"]));

  const globalPlaceholdersAtom = runtime
    .atom(Effect.flatMap(PlaceholderRepository, (repository) => repository.listGlobals))
    .pipe(runtime.factory.withReactivity(["placeholders", "settings"]));
  const saveGlobalPlaceholdersAtom = runtime.fn(
    (input: SaveGlobalPlaceholdersInput) =>
      Effect.flatMap(PlaceholderRepository, (repository) => repository.saveGlobals(input)),
    { reactivityKeys: ["placeholders", "settings"] },
  );

  const fieldIndexAtom = Atom.mapResult(placeholdersAtom, indexFields);

  const catalogPresentationAtom = Atom.mapResult(fieldIndexAtom, catalogFieldPresentation);

  const settingsAtom = runtime
    .atom(Effect.flatMap(SettingsRepository, (repository) => repository.get))
    .pipe(runtime.factory.withReactivity(["settings"]));

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
    .pipe(runtime.factory.withReactivity(["templates"]));

  const templateByIdAtom = Atom.family((templateId: TemplateId) =>
    runtime
      .atom(Effect.flatMap(TemplateRepository, (repository) => repository.get(templateId)))
      .pipe(runtime.factory.withReactivity([`template:${templateId}`])),
  );

  const firsAtom = runtime
    .atom(Effect.flatMap(FirRepository, (repository) => repository.list))
    .pipe(runtime.factory.withReactivity(["firs"]));

  const latestFirsAtom = Atom.mapResult(firsAtom, (firs) =>
    firs.toSorted((first, second) => second.id - first.id).slice(0, LATEST_LIMIT),
  );

  const firByIdAtom = Atom.family((firId: FirId) =>
    runtime
      .atom(Effect.flatMap(FirRepository, (repository) => repository.get(firId)))
      .pipe(runtime.factory.withReactivity(["firs", `fir:${firId}`])),
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
      ),
  );

  const firDisplayModeAtom = Atom.family((_firId: FirId) => Atom.make<FieldDisplayMode>("values"));

  const firPresentationAtom = Atom.family((firId: FirId) =>
    Atom.make((get) =>
      AsyncResult.map(get(firValueContextAtom(firId)), (context) =>
        context.toPresentation(get(firDisplayModeAtom(firId))),
      ),
    ),
  );

  const firDocumentsAtom = Atom.family((firId: FirId) =>
    Atom.optimistic(
      runtime
        .atom(Effect.flatMap(FirDocumentRepository, (repository) => repository.listForFir(firId)))
        .pipe(runtime.factory.withReactivity(["fir-documents", `fir-documents:${firId}`])),
    ),
  );

  const firDocumentByIdAtom = Atom.family((documentId: FirDocumentId | null) =>
    runtime
      .atom(
        documentId === null
          ? Effect.succeed(null)
          : Effect.flatMap(FirDocumentRepository, (repository) => repository.get(documentId)),
      )
      .pipe(runtime.factory.withReactivity([`fir-document:${documentId}`])),
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
    (input: TemplateUpdateInput) =>
      Effect.flatMap(TemplateRepository, (repository) => repository.save(input)),
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

  const saveFirDocumentAtom = runtime.fn((input: FirDocumentSaveInput) =>
    Effect.flatMap(FirDocumentRepository, (repository) => repository.save(input)),
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
    fieldIndexAtom,
    catalogPresentationAtom,
    settingsAtom,
    fieldMarkersAtom,
    saveFieldMarkersAtom,
    templatesAtom,
    templateByIdAtom,
    firsAtom,
    latestFirsAtom,
    firByIdAtom,
    firValueContextAtom,
    firDisplayModeAtom,
    firPresentationAtom,
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

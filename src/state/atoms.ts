import type { SaveGlobalPlaceholdersInput } from "#/lib/global-placeholder";
import { Effect } from "effect";
import { Atom, Reactivity } from "effect/unstable/reactivity";
import type { FirCreateInput, FirId, FirUpdateInput } from "#/lib/fir";
import type { FirDocumentId, TemplateId } from "#/lib/ids";
import {
  indexPlaceholders,
  type PlaceholderCreateInput,
  type PlaceholderUpdateInput,
} from "#/lib/placeholder";
import type {
  AddFirTemplatesInput,
  FirDocumentSaveInput,
  ReorderFirDocumentsInput,
} from "#/lib/fir-document";
import type {
  FirPlaceholderValueRemoveInput,
  FirPlaceholderValueUpsertInput,
  TemplateCreateInput,
  TemplateUpdateInput,
} from "#/lib/templates";
import {
  FirDocumentRepository,
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  TemplateRepository,
  type AppRepositories,
} from "#/repositories/index";
import { appRuntime } from "#/state/app-runtime";

const LATEST_LIMIT = 5;

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

  const templatesAtom = runtime
    .atom(Effect.flatMap(TemplateRepository, (repository) => repository.list))
    .pipe(runtime.factory.withReactivity(["templates"]), Atom.keepAlive);

  const latestTemplatesAtom = Atom.mapResult(templatesAtom, (templates) =>
    templates
      .toSorted((first, second) => {
        const updatedAtComparison = second.updatedAt.localeCompare(first.updatedAt);
        return updatedAtComparison || second.id - first.id;
      })
      .slice(0, LATEST_LIMIT),
  );

  const templateByIdAtom = Atom.family((templateId: TemplateId) =>
    runtime
      .atom(Effect.flatMap(TemplateRepository, (repository) => repository.get(templateId)))
      .pipe(runtime.factory.withReactivity(["templates", `template:${templateId}`])),
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

  const firDocumentsAtom = Atom.family((firId: FirId) =>
    runtime
      .atom(Effect.flatMap(FirDocumentRepository, (repository) => repository.listForFir(firId)))
      .pipe(runtime.factory.withReactivity(["fir-documents", `fir-documents:${firId}`])),
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

  const removeTemplateAtom = runtime.fn(
    (id: TemplateId) => Effect.flatMap(TemplateRepository, (repository) => repository.remove(id)),
    { reactivityKeys: ["templates"] },
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

  const removeFirAtom = runtime.fn(
    (id: FirId) => Effect.flatMap(FirRepository, (repository) => repository.remove(id)),
    { reactivityKeys: ["firs", "fir-values", "fir-documents"] },
  );

  const addFirTemplatesAtom = runtime.fn((input: AddFirTemplatesInput) =>
    Effect.flatMap(FirDocumentRepository, (repository) => repository.addTemplates(input)).pipe(
      Reactivity.mutation(["fir-documents", `fir-documents:${input.firId}`]),
    ),
  );

  const loadPrintDocumentsAtom = runtime.fn((ids: readonly FirDocumentId[]) =>
    Effect.flatMap(FirDocumentRepository, (repository) =>
      Effect.forEach(ids, (id) => repository.get(id)),
    ),
  );

  const saveFirDocumentAtom = runtime.fn((input: FirDocumentSaveInput) =>
    Effect.flatMap(FirDocumentRepository, (repository) => repository.save(input)).pipe(
      Reactivity.mutation([`fir-document:${input.id}`, "fir-documents"]),
    ),
  );

  const reorderFirDocumentsAtom = runtime.fn((input: ReorderFirDocumentsInput) =>
    Effect.flatMap(FirDocumentRepository, (repository) => repository.reorder(input)).pipe(
      Reactivity.mutation(["fir-documents", `fir-documents:${input.firId}`]),
    ),
  );

  const removeFirDocumentAtom = runtime.fn((id: FirDocumentId) =>
    Effect.flatMap(FirDocumentRepository, (repository) => repository.remove(id)).pipe(
      Reactivity.mutation(["fir-documents"]),
    ),
  );

  const upsertFirValueAtom = runtime.fn((input: FirPlaceholderValueUpsertInput) =>
    Effect.flatMap(FirPlaceholderValueRepository, (repository) => repository.upsert(input)).pipe(
      Reactivity.mutation(["fir-values", `fir-values:${input.firId}`]),
    ),
  );

  const removeFirValueAtom = runtime.fn((input: FirPlaceholderValueRemoveInput) =>
    Effect.flatMap(FirPlaceholderValueRepository, (repository) => repository.remove(input)).pipe(
      Reactivity.mutation(["fir-values", `fir-values:${input.firId}`]),
    ),
  );

  return {
    globalPlaceholdersAtom,
    saveGlobalPlaceholdersAtom,
    placeholdersAtom,
    placeholderIndexAtom,
    templatesAtom,
    latestTemplatesAtom,
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
    upsertFirValueAtom,
    removeFirValueAtom,
  };
}

export const atoms = makeAppAtoms(appRuntime);

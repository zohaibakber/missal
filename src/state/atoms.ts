import { Effect } from "effect";
import { Atom, Reactivity } from "effect/unstable/reactivity";
import type { FirCreateInput, FirDocumentUpdateInput, FirId, FirUpdateInput } from "#/lib/fir";
import {
  indexPlaceholders,
  type PlaceholderCreateInput,
  type PlaceholderUpdateInput,
} from "#/lib/placeholder";
import type {
  FirPlaceholderValueRemoveInput,
  FirPlaceholderValueUpsertInput,
  TemplateCreateInput,
  TemplateId,
  TemplateUpdateInput,
} from "#/lib/templates";
import {
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  SettingsRepository,
  TemplateRepository,
  type AppRepositories,
} from "#/repositories/index";
import { appRuntime } from "#/state/app-runtime";

const LATEST_LIMIT = 5;

export function makeAppAtoms(runtime: Atom.AtomRuntime<AppRepositories>) {
  const placeholdersAtom = runtime
    .atom(Effect.flatMap(PlaceholderRepository, (repository) => repository.list))
    .pipe(runtime.factory.withReactivity(["placeholders"]), Atom.keepAlive);

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

  const firsAtom = runtime
    .atom(Effect.flatMap(FirRepository, (repository) => repository.list))
    .pipe(runtime.factory.withReactivity(["firs"]), Atom.keepAlive);

  const latestFirsAtom = Atom.mapResult(firsAtom, (firs) =>
    firs.toSorted((first, second) => second.id - first.id).slice(0, LATEST_LIMIT),
  );

  const firByIdAtom = Atom.family((firId: FirId) =>
    Atom.mapResult(firsAtom, (firs) => firs.find((fir) => fir.id === firId)),
  );

  const firEditorAtom = Atom.family((firId: FirId) =>
    runtime
      .atom(
        Effect.all(
          {
            fir: Effect.flatMap(FirRepository, (repository) => repository.get(firId)),
            templates: Effect.flatMap(TemplateRepository, (repository) => repository.list),
            values: Effect.flatMap(FirPlaceholderValueRepository, (repository) =>
              repository.listForFir(firId),
            ),
            placeholders: Effect.flatMap(PlaceholderRepository, (repository) => repository.list),
            settings: Effect.flatMap(SettingsRepository, (repository) => repository.get),
          },
          { concurrency: "unbounded" },
        ),
      )
      .pipe(
        runtime.factory.withReactivity([
          "firs",
          "templates",
          "placeholders",
          "settings",
          "fir-values",
          `fir-values:${firId}`,
        ]),
      ),
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

  const updateTemplateAtom = runtime.fn(
    (input: TemplateUpdateInput) =>
      Effect.flatMap(TemplateRepository, (repository) => repository.update(input)),
    { reactivityKeys: ["templates"] },
  );

  const removeTemplateAtom = runtime.fn(
    (id: TemplateId) => Effect.flatMap(TemplateRepository, (repository) => repository.remove(id)),
    { reactivityKeys: ["templates", "firs"] },
  );

  const createFirAtom = runtime.fn(
    (input: FirCreateInput) =>
      Effect.flatMap(FirRepository, (repository) => repository.create(input)),
    { reactivityKeys: ["firs"] },
  );

  const updateFirAtom = runtime.fn(
    (input: FirUpdateInput) =>
      Effect.flatMap(FirRepository, (repository) => repository.update(input)),
    { reactivityKeys: ["firs"] },
  );

  const updateFirDocumentAtom = runtime.fn(
    (input: FirDocumentUpdateInput) =>
      Effect.flatMap(FirRepository, (repository) => repository.updateDocument(input)),
    { reactivityKeys: ["firs"] },
  );

  const removeFirAtom = runtime.fn(
    (id: FirId) => Effect.flatMap(FirRepository, (repository) => repository.remove(id)),
    { reactivityKeys: ["firs", "fir-values"] },
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
    placeholdersAtom,
    placeholderIndexAtom,
    templatesAtom,
    latestTemplatesAtom,
    firsAtom,
    latestFirsAtom,
    firByIdAtom,
    firEditorAtom,
    createPlaceholderAtom,
    updatePlaceholderAtom,
    createTemplateAtom,
    updateTemplateAtom,
    removeTemplateAtom,
    createFirAtom,
    updateFirAtom,
    updateFirDocumentAtom,
    removeFirAtom,
    upsertFirValueAtom,
    removeFirValueAtom,
  };
}

export const atoms = makeAppAtoms(appRuntime);

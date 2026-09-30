import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Cause, Effect, Exit, Schema } from "effect";
import { AtomRegistry } from "effect/reactivity";
import {
  decodeStorageRequest,
  encodeStorageResponse,
  type StorageRequest,
  type StorageResponse,
} from "#/electron/storage-contract";
import { DocumentEnvelope, emptyDocumentEnvelope } from "#/lib/document-format";
import { FirDocumentRecord } from "#/lib/fir-document";
import { StorageUnknownOutcome } from "#/lib/storage-errors";
import { TemplateRecord, TemplateSummary } from "#/lib/templates";
import { atoms } from "#/state/atoms";

const timestamp = "2026-09-30T00:00:00.000Z";
const original = emptyDocumentEnvelope();
const edited = new DocumentEnvelope({
  format: original.format,
  version: original.version,
  state: {
    root: {
      type: "root",
      children: [{ type: "paragraph", children: [{ type: "text", text: "Edited" }] }],
    },
  },
});
const currentInput = {
  id: 1,
  name: "Original",
  document: original,
  revision: 1,
  createdAt: timestamp,
  updatedAt: timestamp,
};
const current = Schema.decodeUnknownSync(TemplateRecord)(currentInput);
const summary = Schema.decodeUnknownSync(TemplateSummary)({
  ...currentInput,
  fieldCount: 0,
  previewText: "",
});
let registry: AtomRegistry.AtomRegistry;

beforeEach(() => {
  registry = AtomRegistry.make();
});
afterEach(() => {
  registry.dispose();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function storage(respond: (request: StorageRequest) => StorageResponse) {
  vi.stubGlobal("window", {
    electronStorage: {
      request: async (payload: string) =>
        encodeStorageResponse(respond(await Effect.runPromise(decodeStorageRequest(payload)))),
    },
  });
}

const unknown = (operation: string): StorageResponse => ({
  _tag: "Failure",
  error: new StorageUnknownOutcome({ operation, message: "Acknowledgement lost" }),
});

it.each([
  { name: "Other window", document: edited, revision: 2, saved: false },
  { name: "Edited", document: original, revision: 2, saved: false },
  { name: "Edited", document: edited, revision: 1, saved: false },
  { name: "Edited", document: edited, revision: 3, saved: false },
  { name: "Edited", document: edited, revision: 2, saved: true },
])("reconciles an uncertain template save against $name at revision $revision", async (stored) => {
  let record = current;
  storage((request) => {
    if (request._tag === "Template.save") {
      record = Schema.decodeUnknownSync(TemplateRecord)({ ...currentInput, ...stored });
      return unknown(request._tag);
    }
    if (request._tag === "Template.get") {
      return { _tag: "Success", value: record };
    }
    throw new Error(request._tag);
  });
  const body = atoms.templateByIdAtom(current.id);
  registry.mount(body);
  await Effect.runPromise(AtomRegistry.getResult(registry, body));
  registry.mount(atoms.saveTemplateAtom);
  registry.set(atoms.saveTemplateAtom, { current, document: edited, name: "Edited" });
  const exit = await Effect.runPromiseExit(
    AtomRegistry.getResult(registry, atoms.saveTemplateAtom, { suspendOnWaiting: true }),
  );
  if (stored.saved) {
    expect(exit).toMatchObject({ _tag: "Success", value: { revision: 2 } });
    expect(registry.get(body)).toMatchObject({
      _tag: "Success",
      value: { document: edited, name: "Edited", revision: 2 },
    });
  } else {
    expect(Exit.isFailure(exit) && Cause.squash(exit.cause)).toMatchObject({
      _tag: "StorageUnknownOutcome",
    });
    expect(registry.get(body)).toMatchObject({ _tag: "Success", value: current });
  }
});

it.each([original, edited])(
  "checks the body when reconciling an uncertain FIR document save",
  async (document) => {
    const currentInput = {
      id: 1,
      firId: 1,
      templateId: 1,
      title: "Document",
      sourceTemplateRevision: 1,
      position: 0,
      document: original,
      revision: 1,
      createdAt: timestamp,
      updatedAt: timestamp,
    };
    const current = Schema.decodeUnknownSync(FirDocumentRecord)(currentInput);
    storage((request) => {
      if (request._tag === "FirDocument.save") return unknown(request._tag);
      if (request._tag === "FirDocument.get") {
        return { _tag: "Success", value: { ...currentInput, document, revision: 2 } };
      }
      throw new Error(request._tag);
    });
    registry.mount(atoms.saveFirDocumentAtom);
    registry.set(atoms.saveFirDocumentAtom, { current, document: edited });
    const exit = await Effect.runPromiseExit(
      AtomRegistry.getResult(registry, atoms.saveFirDocumentAtom, { suspendOnWaiting: true }),
    );
    expect(exit._tag).toBe(document === edited ? "Success" : "Failure");
  },
);

it("refreshes the field catalog when an installed pack is already ready", async () => {
  let installed = false;
  storage((request) => {
    switch (request._tag) {
      case "Placeholder.list":
        return {
          _tag: "Success",
          value: installed ? [{ id: 100, label: "Pack field", source: { _tag: "Custom" } }] : [],
        };
      case "Template.packStatus":
        return { _tag: "Success", value: { _tag: "Ready" } };
      case "Template.list":
        return { _tag: "Success", value: [summary] };
      default:
        throw new Error(request._tag);
    }
  });
  registry.mount(atoms.placeholdersAtom);
  expect(await Effect.runPromise(AtomRegistry.getResult(registry, atoms.placeholdersAtom))).toEqual(
    [],
  );
  installed = true;
  registry.mount(atoms.templatesAtom);
  await Effect.runPromise(AtomRegistry.getResult(registry, atoms.templatesAtom));
  expect(
    await Effect.runPromise(
      AtomRegistry.getResult(registry, atoms.placeholdersAtom, { suspendOnWaiting: true }),
    ),
  ).toMatchObject([{ label: "Pack field" }]);
});

it("reads the final template list when installation finishes during a refresh", async () => {
  let installed = false;
  storage((request) => {
    switch (request._tag) {
      case "Template.packStatus":
        return {
          _tag: "Success",
          value: { _tag: installed ? "Ready" : "Synchronizing", done: 0, total: 1 },
        };
      case "Template.list": {
        const value = installed ? [summary] : [];
        installed = true;
        return { _tag: "Success", value };
      }
      default:
        throw new Error(request._tag);
    }
  });
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  registry.mount(atoms.templatesAtom);
  expect(await Effect.runPromise(AtomRegistry.getResult(registry, atoms.templatesAtom))).toEqual(
    [],
  );
  await vi.advanceTimersByTimeAsync(200);
  expect(
    await Effect.runPromise(
      AtomRegistry.getResult(registry, atoms.templatesAtom, { suspendOnWaiting: true }),
    ),
  ).toEqual([summary]);
});

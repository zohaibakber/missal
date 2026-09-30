import "effect/schema/SchemaJITCompiler/enable";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { gzipSync } from "node:zlib";
import { Effect, Schedule, Schema } from "effect";
import { makeStorageRuntime } from "#/electron/storage-runtime";
import { templatePackBodyPath } from "#/lib/bundled-templates";
import { DocumentEnvelope } from "#/lib/document-format";
import { createEmptyFirRecord, FirCreateInput } from "#/lib/fir";
import { AddFirTemplatesInput } from "#/lib/fir-document";
import { PlaceholderId, TemplateId } from "#/lib/ids";
import { FirPlaceholderValueUpsertInput, TemplateCreateInput } from "#/lib/templates";
import {
  FirDocumentRepository,
  FirPlaceholderValueRepository,
  FirRepository,
  PlaceholderRepository,
  TemplateRepository,
} from "#/repositories/index";
import {
  addCounts,
  buildDocument,
  emptyCounts,
  loadSourceCorpus,
  type BuiltDocument,
  type NodeCounts,
} from "./documents";
import { mulberry32, pickInt, pickItem } from "./prng";

const FROZEN_EPOCH_MS = 1_700_000_000_000;
const CUSTOM_FIELD = "خانہ نمبر";

const FIXTURE_PROFILES = {
  small: {
    firs: 100,
    bundledTemplates: 100,
    userTemplates: 0,
    pageMin: 1,
    pageMax: 5,
    extraFieldsPerPage: 1,
  },
  normal: {
    firs: 5_000,
    bundledTemplates: 100,
    userTemplates: 100,
    pageMin: 5,
    pageMax: 20,
    extraFieldsPerPage: 2,
  },
  stress: {
    firs: 25_000,
    bundledTemplates: 100,
    userTemplates: 400,
    pageMin: 50,
    pageMax: 100,
    extraFieldsPerPage: 12,
  },
} as const;

type FixtureName = keyof typeof FIXTURE_PROFILES;

const OFFENCES = ["چوری 379 PPC", "ڈکیتی 392 PPC", "نقب زنی", "قتل 302 PPC", "زیادتی"] as const;
const NAMES = ["محمد علی", "فاطمہ بی بی", "احمد خان", "عائدہ نور", "John Smith"] as const;
const PLACES = ["محلہ شاہ پور", "گلی نمبر 4", "چک نمبر ۱۲", "Thana Road"] as const;
const STATUSES = ["Open", "Under Investigation", "Challan Submitted", "Closed"] as const;

export type FixtureMetadata = {
  readonly fixture: FixtureName;
  readonly seed: number;
  readonly databasePath: string;
  readonly sha256: string;
  readonly sourceEnvelope: string;
  readonly sourceSha256: string;
  readonly counts: {
    readonly firs: number;
    readonly bundledTemplates: number;
    readonly userTemplates: number;
    readonly documents: number;
    readonly placeholders: number;
  };
  readonly byteSizes: {
    readonly database: number;
    readonly documents: number;
    readonly largestDocument: number;
  };
  readonly nodeCounts: NodeCounts;
  readonly pageCounts: { readonly min: number; readonly max: number; readonly total: number };
  readonly images: readonly { readonly width: number; readonly height: number }[];
  readonly probes: {
    readonly firId: number;
    readonly documentId: number;
    readonly documentIds: readonly number[];
    readonly templateSearch: string;
  };
};

export function parseFixtureName(value: string): FixtureName {
  if (value === "small" || value === "normal" || value === "stress") return value;
  throw new Error(`Unknown fixture ${value}. Use small, normal, or stress.`);
}

export function repoRootFromHere() {
  return path.resolve(import.meta.dirname, "../..");
}

export async function generateFixture(options: {
  fixture: FixtureName;
  seed: number;
  outDir?: string;
  repoRoot?: string;
}): Promise<FixtureMetadata> {
  const repoRoot = options.repoRoot ?? repoRootFromHere();
  const profile = FIXTURE_PROFILES[options.fixture];
  const directory = path.resolve(options.outDir ?? path.join(repoRoot, ".perf", options.fixture));
  assertIsolated(directory);
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(directory, { recursive: true });

  const random = mulberry32(options.seed);
  const source = loadSourceCorpus(repoRoot);
  const nodes = emptyCounts();
  let documentBytes = 0;
  let largestDocument = 0;
  let pageTotal = 0;
  let pageMin = Number.POSITIVE_INFINITY;
  let pageMax = 0;
  const images: { width: number; height: number }[] = [];
  const remember = (built: BuiltDocument) => {
    absorb(built, nodes, images);
    documentBytes += built.byteSize;
    largestDocument = Math.max(largestDocument, built.byteSize);
    pageTotal += built.pageCount;
    pageMin = Math.min(pageMin, built.pageCount);
    pageMax = Math.max(pageMax, built.pageCount);
  };

  const builtByName = new Map<string, BuiltDocument>();
  const bundled = [];
  for (let index = 0; index < profile.bundledTemplates; index += 1) {
    const pages = pickInt(random, profile.pageMin, profile.pageMax);
    const pageOffset = pickInt(random, 0, Math.max(0, source.pages.length - 1));
    const name = `بنڈل ${String(index + 1).padStart(3, "0")}`;
    const built = buildDocument({
      repoRoot,
      pages,
      pageOffset,
      extraFieldsPerPage: profile.extraFieldsPerPage,
      label: name,
    });
    bundled.push({ name, built });
    builtByName.set(name, built);
    remember(built);
  }

  const pack = path.join(directory, "pack");
  mkdirSync(path.join(pack, "bodies"), { recursive: true });
  const entries = bundled.map((entry) => {
    const json = Buffer.from(JSON.stringify(entry.built.envelope));
    const bodyHash = createHash("sha256").update(json).digest("hex");
    writeFileSync(path.join(pack, templatePackBodyPath(bodyHash)), gzipSync(json));
    return { name: entry.name, bodyHash, bytes: json.byteLength };
  });
  const packHash = createHash("sha256").update(JSON.stringify(entries)).digest("hex");
  writeFileSync(path.join(pack, "manifest.json"), `${JSON.stringify({ packHash, entries })}\n`);

  const databasePath = path.join(directory, "missal.sqlite");
  const metadata = await withFrozenClock(FROZEN_EPOCH_MS + (options.seed % 1000), async () => {
    const runtime = makeStorageRuntime({
      databasePath,
      migrationsFolder: path.join(repoRoot, "drizzle"),
      bundledTemplatesFolder: pack,
    });
    try {
      const stored = await runtime.runPromise(
        Effect.gen(function* () {
          const templates = yield* TemplateRepository;
          const packStatus = yield* templates.packStatus.pipe(
            Effect.repeat({
              until: (status) => status._tag !== "Synchronizing",
              schedule: Schedule.spaced("50 millis"),
            }),
          );
          if (packStatus._tag !== "Ready") throw new Error("Fixture template pack did not install");
          const placeholders = yield* Effect.flatMap(PlaceholderRepository, (repo) => repo.list);
          const custom = placeholders.find((placeholder) => placeholder.label === CUSTOM_FIELD);
          const bundledRows = yield* templates.list;
          const userIds: TemplateId[] = [];
          for (let index = 0; index < profile.userTemplates; index += 1) {
            const pages = pickInt(random, profile.pageMin, profile.pageMax);
            const pageOffset = pickInt(random, 0, Math.max(0, source.pages.length - 1));
            const name = `صارف ${String(index + 1).padStart(3, "0")}`;
            const built = buildDocument({
              repoRoot,
              pages,
              pageOffset,
              extraFieldsPerPage: profile.extraFieldsPerPage,
              label: name,
            });
            const summary = yield* templates.create(
              new TemplateCreateInput({
                name,
                document: catalogDocument(built.envelope, placeholders),
              }),
            );
            userIds.push(summary.id);
            builtByName.set(name, built);
            remember(built);
          }

          const templateIds = [...bundledRows.map((row) => row.id).toReversed(), ...userIds];
          const firs = yield* FirRepository;
          const documents = yield* FirDocumentRepository;
          const values = yield* FirPlaceholderValueRepository;
          const documentIds: number[] = [];
          let firstFir = 0;
          for (let index = 0; index < profile.firs; index += 1) {
            const fir = yield* firs.create(firInput(options.seed, index, random));
            if (index === 0) firstFir = fir.id;
            const templateId = templateIds[index % templateIds.length] as TemplateId;
            const attached = yield* documents.addTemplates(
              new AddFirTemplatesInput({ firId: fir.id, templateIds: [templateId] }),
            );
            const attachedDocument = attached.find(
              (document) => document.templateId === templateId,
            );
            if (!attachedDocument) throw new Error("Template was not copied onto the FIR");
            documentIds.push(attachedDocument.id);
            const copied = builtByName.get(attachedDocument.title);
            if (copied) remember(copied);
            if (custom) {
              yield* values.upsert(
                new FirPlaceholderValueUpsertInput({
                  firId: fir.id,
                  placeholderId: custom.id,
                  value: `${pickItem(random, NAMES)} — خانہ ${index + 1}`,
                }),
              );
            }
            if ((index + 1) % 500 === 0) {
              console.error(`fixture ${options.fixture}: ${index + 1}/${profile.firs} FIRs`);
            }
          }
          return {
            placeholderCount: placeholders.length,
            firstFir,
            documentIds,
          };
        }),
      );
      return stored;
    } finally {
      await runtime.dispose();
    }
  });

  // WAL salt is random. Checkpoint into the main file before hashing, then drop the sidecars.
  const database = new DatabaseSync(databasePath);
  database.exec("PRAGMA wal_checkpoint(TRUNCATE)");
  database.close();
  rmSync(`${databasePath}-wal`, { force: true });
  rmSync(`${databasePath}-shm`, { force: true });

  const sha256 = await hashFile(databasePath);
  const result: FixtureMetadata = {
    fixture: options.fixture,
    seed: options.seed,
    databasePath,
    sha256,
    sourceEnvelope: source.file,
    sourceSha256: await hashFile(source.file),
    counts: {
      firs: profile.firs,
      bundledTemplates: profile.bundledTemplates,
      userTemplates: profile.userTemplates,
      documents: profile.firs,
      placeholders: metadata.placeholderCount,
    },
    byteSizes: {
      database: statSync(databasePath).size,
      documents: documentBytes,
      largestDocument,
    },
    nodeCounts: nodes,
    pageCounts: {
      min: Number.isFinite(pageMin) ? pageMin : 0,
      max: pageMax,
      total: pageTotal,
    },
    images,
    probes: {
      firId: metadata.firstFir,
      documentId: metadata.documentIds[0] ?? 0,
      documentIds: metadata.documentIds.slice(0, 8),
      templateSearch: "ریمانڈ",
    },
  };
  writeFileSync(path.join(directory, "fixture.json"), `${JSON.stringify(result, null, 2)}\n`);
  return result;
}

function absorb(
  built: {
    byteSize: number;
    nodes: NodeCounts;
    images: readonly { width: number; height: number }[];
  },
  nodes: NodeCounts,
  images: { width: number; height: number }[],
) {
  addCounts(nodes, built.nodes);
  if (images.length < 32) images.push(...built.images.slice(0, 32 - images.length));
}

function catalogDocument(
  envelope: DocumentEnvelope,
  placeholders: readonly { id: PlaceholderId; label: string }[],
) {
  const byLabel = new Map(placeholders.map((placeholder) => [placeholder.label, placeholder.id]));
  const state = JSON.parse(JSON.stringify(envelope.state)) as unknown;
  rewriteFields(state, byLabel);
  return new DocumentEnvelope({
    format: envelope.format,
    version: envelope.version,
    state,
    ...(envelope.pageLayout ? { pageLayout: envelope.pageLayout } : {}),
  });
}

function rewriteFields(value: unknown, byLabel: Map<string, PlaceholderId>) {
  if (Array.isArray(value)) {
    for (const entry of value) rewriteFields(entry, byLabel);
    return;
  }
  if (typeof value !== "object" || value === null) return;
  const node = value as {
    type?: string;
    reference?: { _tag?: string; text?: string; id?: number };
    children?: unknown;
    root?: unknown;
  };
  if (node.type === "field" && node.reference?._tag === "UnresolvedToken") {
    const id = node.reference.text ? byLabel.get(node.reference.text) : undefined;
    if (id !== undefined) node.reference = { _tag: "CatalogField", id };
  }
  if (Array.isArray(node.children)) rewriteFields(node.children, byLabel);
  if (node.root !== undefined) rewriteFields(node.root, byLabel);
}

function firInput(seed: number, index: number, random: () => number) {
  const day = String((index % 28) + 1).padStart(2, "0");
  const accusedCount = pickInt(random, 1, 3);
  const accused = Array.from({ length: accusedCount }, () => {
    const name = pickItem(random, NAMES);
    const place = pickItem(random, PLACES);
    return `${name} ولد ${pickItem(random, NAMES)}، ${place}`;
  });
  const empty = createEmptyFirRecord();
  return Schema.decodeUnknownSync(FirCreateInput)({
    ...empty,
    fir_no: `${seed}-${index + 1}/2026`,
    date: `${day}-09-2026`,
    incident_date: `${day}-09-2026`,
    arrest_date: index % 2 === 0 ? `${day}-09-2026` : "",
    offence: pickItem(random, OFFENCES),
    accused,
    witness: index % 3 === 0 ? [`گواہ ${pickItem(random, NAMES)}`] : [],
    zimni: index % 4 === 0 ? [`ضمنی ${index + 1}`] : [],
    NIC: `35202-${String(1000000 + index).slice(0, 7)}-${(index % 9) + 1}`,
    mobile: `0300-${String(1000000 + index).slice(0, 7)}`,
    investigation_officer: pickItem(random, NAMES),
    status: STATUSES[index % STATUSES.length],
  });
}

function assertIsolated(directory: string) {
  const resolved = path.resolve(directory);
  const forbidden = [path.join(homedir(), ".config", "Missal"), path.join(tmpdir(), "missal-user")];
  for (const dir of forbidden) {
    if (resolved === dir || resolved.startsWith(`${dir}${path.sep}`)) {
      throw new Error(`Refusing to write a benchmark profile at ${resolved}`);
    }
  }
}

async function withFrozenClock<T>(millis: number, run: () => Promise<T>) {
  // `new Date()` reads the system clock, not `Date.now()`, and Drizzle stamps migrations that way.
  const OriginalDate = Date;
  class FrozenDate extends OriginalDate {
    constructor(...args: unknown[]) {
      if (args.length === 0) super(millis);
      else super(...(args as ConstructorParameters<typeof Date>));
    }
  }
  FrozenDate.now = () => millis;
  globalThis.Date = FrozenDate as DateConstructor;
  try {
    return await run();
  } finally {
    globalThis.Date = OriginalDate;
  }
}

function hashFile(file: string) {
  return new Promise<string>((resolve, reject) => {
    const hash = createHash("sha256");
    createReadStream(file)
      .on("data", (chunk) => hash.update(chunk))
      .on("error", reject)
      .on("end", () => resolve(hash.digest("hex")));
  });
}

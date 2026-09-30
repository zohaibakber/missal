import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";

const FIELD_LABELS = [
  "مقدمہ نمبر",
  "Date FIR",
  "تاریخ ووقت وقوعہ",
  "تاریخ گرفتاری",
  "جرم",
  "نام ملزم و سکونت",
  "گواہان",
  "شناختی کارڈ",
  "موبائل",
  "تفتیشی",
  "تھانہ نام",
  "ضلع نام",
  "SHO نام",
  "DSP نام",
  "ضمنی",
  "گواہان 1",
  "گواہان 2",
  "مدعی مقدمہ",
  "حلیہ ملزم",
  "مختصر حالات",
  "خانہ نمبر",
] as const;

export type NodeCounts = {
  total: number;
  text: number;
  field: number;
  paragraph: number;
  image: number;
};

export type ImageSize = { readonly width: number; readonly height: number };

export type BuiltDocument = {
  readonly envelope: DocumentEnvelope;
  readonly byteSize: number;
  readonly pageCount: number;
  readonly nodes: NodeCounts;
  readonly images: readonly ImageSize[];
};

type SourcePage = readonly unknown[];

type SourceCorpus = {
  readonly file: string;
  readonly pages: readonly SourcePage[];
  readonly pageLayout: DocumentEnvelope["pageLayout"];
};

let corpus: SourceCorpus | undefined;

export function loadSourceCorpus(repoRoot: string): SourceCorpus {
  if (corpus) return corpus;
  const folder = path.join(repoRoot, "bundled-templates");
  const fileName = readdirSync(folder)
    .filter((file) => file.endsWith(".json") && file !== "index.json")
    .sort()[0];
  if (!fileName) throw new Error("bundled-templates has no document envelope");
  const file = path.join(folder, fileName);
  const envelope = Schema.decodeUnknownSync(DocumentEnvelope)(
    JSON.parse(readFileSync(file, "utf8")) as unknown,
  );
  const root = (envelope.state as { root?: { children?: unknown[] } }).root;
  const children = root?.children;
  if (!children) throw new Error("Bundled envelope has no lexical root");
  const pages: SourcePage[] = [];
  let page: unknown[] = [];
  for (const child of children) {
    if (isRecord(child) && child.type === "pagebreak") {
      pages.push(page);
      page = [];
    } else {
      page.push(child);
    }
  }
  pages.push(page);
  corpus = { file, pages, pageLayout: envelope.pageLayout };
  return corpus;
}

export function buildDocument(options: {
  repoRoot: string;
  pages: number;
  pageOffset: number;
  extraFieldsPerPage: number;
  label: string;
}): BuiltDocument {
  const source = loadSourceCorpus(options.repoRoot);
  const blocks: unknown[] = [bidiParagraph(options.label), ltrParagraph()];
  for (let page = 0; page < options.pages; page += 1) {
    const sourcePage = source.pages[(options.pageOffset + page) % source.pages.length] ?? [];
    for (const block of sourcePage) blocks.push(structuredClone(block));
    if (options.extraFieldsPerPage > 0) {
      blocks.push(fieldParagraph(options.extraFieldsPerPage, page));
    }
    if (page + 1 < options.pages) blocks.push({ type: "pagebreak", version: 1 });
  }
  const state = {
    root: {
      children: blocks,
      direction: "rtl" as const,
      format: "",
      indent: 0,
      type: "root",
      version: 1,
    },
  };
  const envelope = new DocumentEnvelope({
    format: "missal-lexical",
    version: 1,
    state,
    ...(source.pageLayout ? { pageLayout: source.pageLayout } : {}),
  });
  const nodes = emptyCounts();
  const images: ImageSize[] = [];
  collect(state, nodes, images);
  return {
    envelope,
    byteSize: Buffer.byteLength(JSON.stringify(envelope)),
    pageCount: options.pages,
    nodes,
    images,
  };
}

export function emptyCounts(): NodeCounts {
  return { total: 0, text: 0, field: 0, paragraph: 0, image: 0 };
}

export function addCounts(target: NodeCounts, extra: NodeCounts) {
  target.total += extra.total;
  target.text += extra.text;
  target.field += extra.field;
  target.paragraph += extra.paragraph;
  target.image += extra.image;
}

function bidiParagraph(label: string) {
  return {
    type: "paragraph",
    version: 1,
    direction: "rtl",
    format: "justify",
    indent: 0,
    textFormat: 0,
    textStyle: "",
    children: [
      {
        detail: 0,
        format: 1,
        mode: "normal",
        style: 'font-family: "Jameel Noori Nastaleeq"; direction: rtl; unicode-bidi: isolate;',
        text: `ریمانڈ — ${label} `,
        type: "text",
        version: 1,
      },
      {
        detail: 0,
        format: 8,
        mode: "normal",
        style: "font-family: Calibri; direction: ltr; unicode-bidi: isolate;",
        text: "FIR 12/2026 Section 379 PPC",
        type: "text",
        version: 1,
      },
      {
        type: "field",
        version: 1,
        reference: { _tag: "UnresolvedToken", text: "جرم" },
      },
      {
        type: "field",
        version: 1,
        reference: { _tag: "UnresolvedToken", text: "خانہ نمبر" },
      },
    ],
  };
}

function ltrParagraph() {
  return {
    type: "paragraph",
    version: 1,
    direction: "ltr",
    format: "start",
    indent: 0,
    textFormat: 0,
    textStyle: "",
    children: [
      {
        detail: 0,
        format: 0,
        mode: "normal",
        style: "",
        text: "Station copy / تھانہ کاپی ۱۲۳",
        type: "text",
        version: 1,
      },
    ],
  };
}

function fieldParagraph(count: number, page: number) {
  const children = [];
  for (let index = 0; index < count; index += 1) {
    children.push({
      type: "field",
      version: 1,
      reference: {
        _tag: "UnresolvedToken",
        text: FIELD_LABELS[(page + index) % FIELD_LABELS.length],
      },
    });
  }
  return {
    type: "paragraph",
    version: 1,
    direction: "rtl",
    format: "",
    indent: 0,
    textFormat: 0,
    textStyle: "",
    children,
  };
}

function collect(value: unknown, counts: NodeCounts, images: ImageSize[]) {
  if (Array.isArray(value)) {
    for (const entry of value) collect(entry, counts, images);
    return;
  }
  if (!isRecord(value)) return;
  if (typeof value.type === "string") {
    counts.total += 1;
    if (value.type === "text") counts.text += 1;
    else if (value.type === "field") counts.field += 1;
    else if (value.type === "paragraph") counts.paragraph += 1;
    else if (value.type === "image") {
      counts.image += 1;
      const width = value.width;
      const height = value.height;
      if (typeof width === "number" && typeof height === "number") images.push({ width, height });
    }
  }
  if (Array.isArray(value.children)) collect(value.children, counts, images);
  if (value.root !== undefined) collect(value.root, counts, images);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

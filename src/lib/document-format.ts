import { Match, Schema, SchemaGetter } from "effect";
import { FieldReference } from "#/lib/field";

export const DOCUMENT_FORMAT = "missal-lexical" as const;
export const DOCUMENT_FORMAT_VERSION = 2 as const;
const PREVIEW_TEXT_LIMIT = 280;

const DocumentFormat = Schema.Literal(DOCUMENT_FORMAT);
const DocumentFormatVersion = Schema.Literal(DOCUMENT_FORMAT_VERSION);

const SerializedLexicalState = Schema.Unknown;

const PageDimension = Schema.Number.pipe(Schema.check(Schema.isGreaterThan(0)));
const PageMargin = Schema.Number.pipe(Schema.check(Schema.isGreaterThanOrEqualTo(0)));

export class PageLayout extends Schema.Class<PageLayout>("PageLayout")({
  widthMm: PageDimension,
  heightMm: PageDimension,
  marginTopMm: PageMargin,
  marginRightMm: PageMargin,
  marginBottomMm: PageMargin,
  marginLeftMm: PageMargin,
}) {}

export class DocumentEnvelope extends Schema.Class<DocumentEnvelope>("DocumentEnvelope")({
  format: DocumentFormat,
  version: DocumentFormatVersion,
  styles: Schema.Array(Schema.String),
  state: SerializedLexicalState,
  pageLayout: Schema.optionalKey(PageLayout),
}) {}

const STYLE_KEYS: ReadonlySet<string> = new Set(["style", "textStyle", "importedStyle"]);

function mapStyles(value: unknown, map: (style: string | number) => string | number): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => mapStyles(item, map));
  }

  if (value === null || typeof value !== "object") {
    return value;
  }

  const result: Record<string, unknown> = {};
  for (const [key, child] of Object.entries(value)) {
    if (child === undefined) continue;
    result[key] =
      STYLE_KEYS.has(key) && (typeof child === "string" || typeof child === "number")
        ? map(child)
        : mapStyles(child, map);
  }
  return result;
}

export function normalizeDocumentStyles(lexicalState: unknown) {
  const styles: string[] = [];
  const indexes = new Map<string, number>();
  const state = mapStyles(lexicalState, (style) => {
    if (typeof style === "number" || style === "") return style;
    let index = indexes.get(style);
    if (index === undefined) {
      index = styles.push(style) - 1;
      indexes.set(style, index);
    }
    return index;
  });
  return { styles, state };
}

export function expandDocumentStyles({
  styles,
  state,
}: {
  readonly styles: readonly string[];
  readonly state: unknown;
}): unknown {
  return mapStyles(state, (style) => (typeof style === "number" ? (styles[style] ?? "") : style));
}

const DocumentEnvelopeV1 = Schema.Struct({
  format: DocumentFormat,
  version: Schema.Literal(1),
  state: SerializedLexicalState,
  pageLayout: Schema.optionalKey(PageLayout),
});

export const StoredDocumentEnvelope = Schema.Union([
  DocumentEnvelope,
  DocumentEnvelopeV1.pipe(
    Schema.decodeTo(DocumentEnvelope, {
      decode: SchemaGetter.transform(({ format, pageLayout, state }) => ({
        format,
        version: DOCUMENT_FORMAT_VERSION,
        ...normalizeDocumentStyles(state),
        ...(pageLayout ? { pageLayout } : {}),
      })),
      encode: SchemaGetter.transform((envelope) => ({
        format: envelope.format,
        version: 1 as const,
        state: expandDocumentStyles(envelope),
        ...(envelope.pageLayout ? { pageLayout: envelope.pageLayout } : {}),
      })),
    }),
  ),
]);

type SerializedLexicalNode = {
  readonly type?: string;
  readonly text?: string;
  readonly children?: readonly unknown[];
  readonly reference?: unknown;
};

const EMPTY_LEXICAL_STATE = {
  root: {
    children: [
      {
        children: [],
        direction: "rtl" as const,
        format: "",
        indent: 0,
        textFormat: 0,
        textStyle: "",
        type: "paragraph",
        version: 1,
      },
    ],
    direction: "rtl" as const,
    format: "",
    indent: 0,
    type: "root",
    version: 1,
  },
};

export function emptyDocumentEnvelope() {
  return new DocumentEnvelope({
    format: DOCUMENT_FORMAT,
    version: DOCUMENT_FORMAT_VERSION,
    styles: [],
    state: EMPTY_LEXICAL_STATE,
  });
}

export class DocumentProjections extends Schema.Class<DocumentProjections>("DocumentProjections")({
  plainText: Schema.String,
  previewText: Schema.String,
  fieldReferences: Schema.Array(FieldReference),
  fieldCount: Schema.Int,
}) {}

export function projectDocument(envelope: DocumentEnvelope): DocumentProjections {
  const texts: string[] = [];
  const fieldReferences: FieldReference[] = [];
  const seen = new Set<string>();

  visitSerializedNode(envelope.state, texts, fieldReferences, seen);

  const plainText = texts.join("").replace(/\s+/g, " ").trim();
  return new DocumentProjections({
    fieldCount: fieldReferences.length,
    fieldReferences,
    plainText,
    previewText: plainText.slice(0, PREVIEW_TEXT_LIMIT),
  });
}

export function documentWriteColumns(envelope: DocumentEnvelope) {
  const projections = projectDocument(envelope);
  return {
    document: envelope,
    fieldCount: projections.fieldCount,
    fieldReferences: [...projections.fieldReferences],
    plainText: projections.plainText,
    previewText: projections.previewText,
  };
}

function visitSerializedNode(
  value: unknown,
  texts: string[],
  fieldReferences: FieldReference[],
  seen: Set<string>,
) {
  if (typeof value !== "object" || value === null) {
    return;
  }

  const node = value as SerializedLexicalNode;

  if (node.type === "text" && typeof node.text === "string") {
    texts.push(node.text);
  }

  if (node.type === "linebreak") {
    texts.push("\n");
  }

  if (node.type === "field") {
    const reference = Schema.decodeUnknownOption(FieldReference)(node.reference);
    if (reference._tag === "Some") {
      const key = fieldReferenceKey(reference.value);
      if (!seen.has(key)) {
        seen.add(key);
        fieldReferences.push(reference.value);
      }
    }
  }

  if (Array.isArray(node.children)) {
    for (const child of node.children) {
      visitSerializedNode(child, texts, fieldReferences, seen);
    }
  }

  if ("root" in (value as Record<string, unknown>)) {
    visitSerializedNode((value as { root: unknown }).root, texts, fieldReferences, seen);
  }
}

function fieldReferenceKey(reference: FieldReference) {
  return Match.valueTags(reference, {
    CatalogField: ({ id }) => `id:${id}`,
    FirField: ({ property, index }) => `fir:${property}:${index ?? ""}`,
    UnresolvedToken: ({ text }) => `token:${text}`,
  });
}

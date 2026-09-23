import { Match, Schema } from "effect";
import { FieldReference } from "#/lib/field";

export const DOCUMENT_FORMAT = "missal-lexical" as const;
export const DOCUMENT_FORMAT_VERSION = 1 as const;
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
  state: SerializedLexicalState,
  pageLayout: Schema.optionalKey(PageLayout),
}) {}

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
    UnresolvedToken: ({ text }) => `token:${text}`,
  });
}

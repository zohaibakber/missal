// Runs inside a hidden Electron window so Word files convert exactly as the app's "Upload Word".
import { Schema } from "effect";
import JSZip from "jszip";
import { createEditor } from "lexical";
import { EDITOR_HTML_CONFIG } from "#/editor/html-config";
import { EDITOR_NODES, EDITOR_THEME } from "#/editor/nodes/registry";
import { attachEditorSession } from "#/editor/session";
import { fieldNameFromToken, mapDocumentFields } from "#/lib/bundled-templates";
import { DocumentEnvelope, emptyDocumentEnvelope, projectDocument } from "#/lib/document-format";
import { catalogFieldPresentation, CustomSource, UnresolvedTokenReference } from "#/lib/field";
import { PlaceholderId } from "#/lib/ids";
import {
  createDefaultPlaceholders,
  indexPlaceholders,
  Placeholder,
  resolvePlaceholder,
} from "#/lib/placeholder";
import { DEFAULT_FIELD_MARKERS } from "#/lib/settings";

const WORD_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const markers = DEFAULT_FIELD_MARKERS;
const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const tokenPattern = new RegExp(
  `${escape(markers.open)}([^\\n]{1,80}?)${escape(markers.close)}`,
  "g",
);

export type ConvertedTemplate = {
  document: unknown;
  fieldNames: string[];
  newFieldNames: string[];
  unconvertedTokens: string[];
  notices: string[];
};

/** Every field token written in the document's paragraphs, headers and footers. */
async function tokensIn(bytes: Uint8Array) {
  const archive = await JSZip.loadAsync(bytes);
  const tokens = new Set<string>();
  for (const entry of Object.values(archive.files)) {
    if (!/^word\/(?:document|header\d+|footer\d+)\.xml$/.test(entry.name)) continue;
    const xml = new DOMParser().parseFromString(await entry.async("string"), "application/xml");
    for (const paragraph of xml.getElementsByTagNameNS(WORD_NS, "p")) {
      const text = [...paragraph.getElementsByTagNameNS(WORD_NS, "t")]
        .map((node) => node.textContent ?? "")
        .join("");
      for (const [, token] of text.matchAll(tokenPattern)) if (token) tokens.add(token);
    }
  }
  return tokens;
}

async function convertDocx(base64: string, fileName: string): Promise<ConvertedTemplate> {
  const bytes = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
  // Names the default catalog lacks become fields here and custom fields when the pack installs.
  const catalog = createDefaultPlaceholders();
  const newFieldNames: string[] = [];
  let index = indexPlaceholders(catalog);
  for (const token of await tokensIn(bytes)) {
    if (resolvePlaceholder(token, index)) continue;
    const label = fieldNameFromToken(token);
    newFieldNames.push(label);
    catalog.push(
      new Placeholder({
        id: PlaceholderId.make(catalog.length + 1),
        label,
        source: CustomSource.make({}),
      }),
    );
    index = indexPlaceholders(catalog);
  }

  const editor = createEditor({
    namespace: "template-pack",
    html: EDITOR_HTML_CONFIG,
    nodes: [...EDITOR_NODES],
    theme: EDITOR_THEME,
    onError: (error) => {
      throw error;
    },
  });
  const root = document.createElement("div");
  document.body.append(root);
  editor.setRootElement(root);
  const session = attachEditorSession(editor, {
    getPlaceholderIndex: () => index,
    getFieldMarkers: () => markers,
    presentation: catalogFieldPresentation(index),
  });
  try {
    session.loadEnvelope(emptyDocumentEnvelope());
    const notices = await session.importDocx(new File([bytes], fileName));
    const { envelope } = session.captureEnvelope();
    const labels = new Map(catalog.map((field) => [field.id, field.label]));
    const byName = mapDocumentFields(envelope, (reference) =>
      reference._tag === "CatalogField"
        ? UnresolvedTokenReference.make({
            text: labels.get(reference.id) ?? String(reference.id),
          })
        : reference,
    );
    const projections = projectDocument(byName);
    return {
      document: Schema.encodeSync(DocumentEnvelope)(byName),
      fieldNames: projections.fieldReferences.flatMap((reference) =>
        reference._tag === "UnresolvedToken" ? [reference.text] : [],
      ),
      newFieldNames,
      unconvertedTokens: [...projections.plainText.matchAll(tokenPattern)].map(([token]) => token),
      notices,
    };
  } finally {
    session.dispose();
    editor.setRootElement(null);
    root.remove();
  }
}

Object.assign(globalThis, { convertDocx });

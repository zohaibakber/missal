export const URDU_FONT_FAMILY = "Jameel Noori Nastaleeq";
export const URDU_FONT_URL = "/Jameel%20Noori%20Nastaleeq.ttf";
import { Match, Schema } from "effect";
import { DocumentEnvelope, type PageLayout } from "#/lib/document-format";
import { FirDocumentId } from "#/lib/ids";

export const OutputJobStatus = Schema.Literals([
  "success",
  "cancelled",
  "emptySelection",
  "unresolvedFields",
  "preparationFailed",
  "rendererFailed",
  "fileWriteFailed",
  "dialogDismissed",
]);

export type OutputJobStatus = typeof OutputJobStatus.Type;

export class UnresolvedFieldNotice extends Schema.Class<UnresolvedFieldNotice>(
  "UnresolvedFieldNotice",
)({
  documentId: FirDocumentId,
  label: Schema.String,
  token: Schema.String,
}) {}

export class OutputDocumentOverride extends Schema.Class<OutputDocumentOverride>(
  "OutputDocumentOverride",
)({
  documentId: FirDocumentId,
  document: DocumentEnvelope,
}) {}

export class OutputSelection extends Schema.Class<OutputSelection>("OutputSelection")({
  documentIds: Schema.Array(FirDocumentId).pipe(Schema.check(Schema.isMinLength(1))),
  activeOverride: Schema.optionalKey(OutputDocumentOverride),
}) {}

export class OutputJobResult extends Schema.Class<OutputJobResult>("OutputJobResult")({
  status: OutputJobStatus,
  message: Schema.optionalKey(Schema.String),
  unresolved: Schema.optionalKey(Schema.Array(UnresolvedFieldNotice)),
}) {}

export function escapeOutputHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

type SectionPage = { readonly name: string; readonly layout: PageLayout };

export function documentSectionHtml(html: string, startOnNewPage: boolean, page?: SectionPage) {
  const pageClass = startOnNewPage ? " missal-print-break" : "";
  // Side margins are padding, not @page margins: Chromium clips anything drawn in the page margin,
  // and Word documents often indent text and tables into it.
  const pageStyle = page
    ? ` style="page: ${escapeOutputHtml(page.name)}; padding: 0 ${page.layout.marginRightMm}mm 0 ${page.layout.marginLeftMm}mm"`
    : "";
  return `<section class="missal-print-document${pageClass}"${pageStyle}>${html}</section>`;
}

export function previewSectionHtml(previewText: string, startOnNewPage: boolean) {
  return documentSectionHtml(`<p>${escapeOutputHtml(previewText)}</p>`, startOnNewPage);
}

export function printablePacketHtml(content: string, title: string, pageRules = "") {
  return `<!doctype html>
<html lang="ur" dir="rtl">
  <head>
    <meta charset="utf-8" />
    <title>${escapeOutputHtml(title)}</title>
    <style>
      @font-face {
        font-family: "Jameel Noori Nastaleeq";
        src: url("${escapeOutputHtml(URDU_FONT_URL)}") format("truetype");
        font-display: swap;
      }
      @page { size: A4; margin: 18mm 0; }
      ${pageRules}
      body {
        margin: 0;
        color: #000;
        font-family: "Jameel Noori Nastaleeq", serif;
        font-size: 20px;
        line-height: 2.4;
        print-color-adjust: exact;
        -webkit-print-color-adjust: exact;
      }
      .missal-print-document {
        direction: rtl; text-align: right; unicode-bidi: isolate; padding: 0 18mm;
      }
      .missal-print-document h1, .missal-print-document h2, .missal-print-document h3,
      .missal-print-document h4, .missal-print-document h5, .missal-print-document h6,
      .missal-print-document blockquote, .missal-print-document ul, .missal-print-document ol { margin: 0; }
      .missal-print-document p { margin: 0 0 8px; white-space: pre-wrap; overflow-wrap: anywhere; }
      .missal-print-document table {
        width: 100%; max-width: 100%; border-collapse: collapse; table-layout: fixed;
        break-inside: auto;
      }
      .missal-print-document td, .missal-print-document th {
        min-width: 0; border: 1px solid #000; padding: 4px 5.6px;
        overflow-wrap: anywhere;
      }
      .missal-print-document th { font-weight: inherit; text-align: inherit; }
      .missal-print-document tr { break-inside: auto; }
      .missal-print-document thead { display: table-header-group; }
      .missal-print-document img { max-width: 100%; height: auto; }
      .missal-print-document ul { list-style: disc; padding-inline-start: 24px; }
      .missal-print-document ol { list-style: decimal; padding-inline-start: 24px; }
      .missal-print-document h1 { font-size: 2em; }
      .missal-print-document h2 { font-size: 1.5em; }
      .missal-print-document h3 { font-size: 1.25em; }
      .missal-print-document blockquote { border-inline-start: 3px solid #000; padding-inline-start: 16px; }
      .missal-rtl { direction: rtl; }
      .missal-ltr { direction: ltr; }
      .missal-text-bold { font-weight: 700; }
      .missal-text-italic { font-style: italic; }
      .missal-text-underline { text-decoration: underline; }
      .missal-text-strike { text-decoration: line-through; }
      .missal-text-underline.missal-text-strike { text-decoration: underline line-through; }
      .missal-print-break { break-before: page; page-break-before: always; }
      .missal-page-break { break-before: page; page-break-before: always; height: 0; margin: 0; border: 0; }
      .missal-field {
        color: inherit;
        background: transparent;
        font: inherit;
        unicode-bidi: isolate;
        white-space: pre-wrap;
      }
    </style>
  </head>
  <body>${content}</body>
</html>`;
}

export type PrintPacketSection =
  | { readonly _tag: "Html"; readonly html: string; readonly pageLayout?: PageLayout }
  | { readonly _tag: "Preview"; readonly previewText: string };

export function printPacketFromSections(sections: readonly PrintPacketSection[], title: string) {
  return printablePacketHtml(
    sections
      .map((section, index) =>
        Match.valueTags(section, {
          Html: ({ html, pageLayout }) =>
            documentSectionHtml(
              html,
              index > 0,
              pageLayout ? { name: `missalDocument${index}`, layout: pageLayout } : undefined,
            ),
          Preview: ({ previewText }) => previewSectionHtml(previewText, index > 0),
        }),
      )
      .join(""),
    title,
    sections
      .map((section, index) => {
        if (section._tag !== "Html" || !section.pageLayout) return "";
        const page = section.pageLayout;
        return `@page missalDocument${index} { size: ${page.widthMm}mm ${page.heightMm}mm; margin: ${page.marginTopMm}mm 0 ${page.marginBottomMm}mm 0; }`;
      })
      .join("\n"),
  );
}

const urduFontUrl = "/Jameel%20Noori%20Nastaleeq.ttf";
import { Match, Schema } from "effect";
import { DocumentEnvelope } from "#/lib/document-format";
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

export function documentSectionHtml(html: string, startOnNewPage: boolean) {
  const pageClass = startOnNewPage ? " missal-print-break" : "";
  return `<section class="missal-print-document${pageClass}">${html}</section>`;
}

export function previewSectionHtml(previewText: string, startOnNewPage: boolean) {
  return documentSectionHtml(`<p>${escapeOutputHtml(previewText)}</p>`, startOnNewPage);
}

export function printablePacketHtml(content: string, title: string) {
  return `<!doctype html>
<html lang="ur" dir="rtl">
  <head>
    <meta charset="utf-8" />
    <title>${escapeOutputHtml(title)}</title>
    <style>
      @font-face {
        font-family: "Jameel Noori Nastaleeq";
        src: url("${escapeOutputHtml(urduFontUrl)}") format("truetype");
        font-display: swap;
      }
      @page { size: A4; margin: 18mm; }
      body {
        margin: 0;
        color: #111827;
        font-family: "Jameel Noori Nastaleeq", serif;
        font-size: 13pt;
        line-height: 2;
      }
      .missal-print-document { direction: rtl; text-align: right; unicode-bidi: isolate; }
      .missal-print-break { break-before: page; page-break-before: always; }
      .missal-page-break { break-before: page; page-break-before: always; height: 0; }
      .missal-field {
        color: #1d4ed8;
        background: #eff6ff;
        unicode-bidi: isolate;
        white-space: pre-wrap;
      }
    </style>
  </head>
  <body>${content}</body>
</html>`;
}

export type PrintPacketSection =
  | { readonly _tag: "Html"; readonly html: string }
  | { readonly _tag: "Preview"; readonly previewText: string };

export function printPacketFromSections(sections: readonly PrintPacketSection[], title: string) {
  return printablePacketHtml(
    sections
      .map((section, index) =>
        Match.valueTags(section, {
          Html: ({ html }) => documentSectionHtml(html, index > 0),
          Preview: ({ previewText }) => previewSectionHtml(previewText, index > 0),
        }),
      )
      .join(""),
    title,
  );
}

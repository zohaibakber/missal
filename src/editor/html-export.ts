import { $generateHtmlFromNodes } from "@lexical/html";
import { createEditor } from "lexical";
import { Match } from "effect";
import { EDITOR_HTML_CONFIG } from "#/editor/html-config";
import { EDITOR_NODES, EDITOR_THEME } from "#/editor/nodes/registry";
import { loadEnvelopeIntoEditor } from "#/editor/envelope";
import { setEditorPresentation } from "#/editor/presentation-context";
import type { FieldPresentationContext } from "#/lib/field";
import type { DocumentEnvelope } from "#/lib/document-format";
import { printPacketFromSections, type PrintPacketSection } from "#/lib/output";

export type EnvelopePrintSection =
  | {
      readonly _tag: "Envelope";
      readonly envelope: DocumentEnvelope;
      readonly presentation: FieldPresentationContext;
    }
  | { readonly _tag: "Preview"; readonly previewText: string };

export function envelopeToHtml(
  envelope: DocumentEnvelope,
  presentation?: FieldPresentationContext,
) {
  const editor = createEditor({
    namespace: "missal-html-export",
    html: EDITOR_HTML_CONFIG,
    nodes: [...EDITOR_NODES],
    onError: () => undefined,
    theme: EDITOR_THEME,
  });
  if (presentation) {
    setEditorPresentation(editor, presentation);
  }
  loadEnvelopeIntoEditor(editor, envelope);
  return editor.getEditorState().read(() => $generateHtmlFromNodes(editor));
}

export type PrintPacket = { readonly html: string; readonly title: string };

/** The complete, self-contained HTML that is both previewed and sent to the printer. */
export function envelopePrintPacket(
  sections: readonly EnvelopePrintSection[],
  title: string,
): PrintPacket {
  return { html: printPacketFromSections(htmlSectionsFrom(sections), title), title };
}

export function printPacket({ html, title }: PrintPacket) {
  const existingFrame = document.getElementById("fir-print-frame");
  existingFrame?.remove();

  const frame = document.createElement("iframe");
  frame.id = "fir-print-frame";
  frame.title = title;
  frame.setAttribute("aria-hidden", "true");
  frame.style.position = "fixed";
  frame.style.insetInlineStart = "-10000px";
  frame.style.width = "210mm";
  frame.style.height = "297mm";
  frame.style.border = "0";

  document.body.append(frame);
  const frameWindow = frame.contentWindow;
  const frameDocument = frame.contentDocument ?? frameWindow?.document;
  if (!frameWindow || !frameDocument) {
    frame.remove();
    return false;
  }

  frameWindow.addEventListener(
    "afterprint",
    () => {
      window.setTimeout(() => frame.remove(), 500);
    },
    { once: true },
  );
  frameDocument.open();
  frameDocument.write(html);
  frameDocument.close();

  const waitForImages = Promise.all(
    Array.from(frameDocument.images).map((image) =>
      image.complete
        ? Promise.resolve()
        : new Promise<void>((resolve) => {
            image.addEventListener("load", () => resolve(), { once: true });
            image.addEventListener("error", () => resolve(), { once: true });
          }),
    ),
  );
  // Request fonts after the written document has layout, including fonts in imported runs.
  frameDocument.body.getBoundingClientRect();
  const waitForFonts = frameDocument.fonts.ready;

  Promise.all([waitForFonts, waitForImages])
    .catch(() => undefined)
    .finally(() => {
      window.setTimeout(() => {
        frameWindow.focus();
        frameWindow.print();
      }, 50);
    });

  return true;
}

function htmlSectionsFrom(sections: readonly EnvelopePrintSection[]): PrintPacketSection[] {
  return sections.map((section) =>
    Match.valueTags(section, {
      Envelope: ({ envelope, presentation }) => ({
        _tag: "Html" as const,
        html: envelopeToHtml(envelope, presentation),
        pageLayout: envelope.pageLayout,
      }),
      Preview: ({ previewText }) => ({ _tag: "Preview" as const, previewText }),
    }),
  );
}

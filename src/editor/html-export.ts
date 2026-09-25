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

// Chromium's reason when the user closes the print dialog without printing.
const PRINT_CANCELED = "Print job canceled";

/**
 * Prints a packet from the hidden window that laid out its preview, so the app's own renderer
 * never lays the pages out again. Resolves an error message, or null once printed or canceled.
 */
export async function printPacket({ html }: PrintPacket): Promise<string | null> {
  const api = window.electronPrint;
  if (!api) return "Printing is only available in the Missal desktop app.";
  try {
    const result = await api.print(html);
    if (result.printed || result.failureReason === PRINT_CANCELED) return null;
    return result.failureReason ?? "Unable to print";
  } catch {
    return "Unable to prepare print view";
  }
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

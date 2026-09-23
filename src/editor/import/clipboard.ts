import { COMMAND_PRIORITY_HIGH, PASTE_COMMAND, type LexicalEditor } from "lexical";
import type { ImportNotice } from "#/editor/import/convert";
import type { PageLayout } from "#/lib/document-format";

export function registerClipboardImport(
  editor: LexicalEditor,
  {
    onNotices,
    onPageLayout,
  }: {
    onNotices?: (notices: ImportNotice[]) => void;
    onPageLayout?: (pageLayout: PageLayout) => void;
  } = {},
) {
  return editor.registerCommand(
    PASTE_COMMAND,
    (event) => {
      if (!(event instanceof ClipboardEvent) || !event.clipboardData) {
        return false;
      }

      const html = event.clipboardData.getData("text/html");
      if (!html) {
        return false;
      }

      event.preventDefault();
      void Promise.all([
        import("#/editor/import/word-html"),
        import("#/editor/import/convert"),
      ]).then(async ([{ prepareFontsFor, readWordPageLayout }, { insertSanitizedHtml }]) => {
        const pageLayout = readWordPageLayout(html);
        if (pageLayout) onPageLayout?.(pageLayout);
        await prepareFontsFor(html);
        const notices = insertSanitizedHtml(editor, html);
        if (notices.length) onNotices?.(notices);
      });

      return true;
    },
    COMMAND_PRIORITY_HIGH,
  );
}

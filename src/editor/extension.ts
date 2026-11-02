import { HorizontalRuleExtension } from "@lexical/extension";
import { HistoryExtension } from "@lexical/history";
import { LinkExtension } from "@lexical/link";
import { ListExtension } from "@lexical/list";
import { RichTextExtension } from "@lexical/rich-text";
import { TableExtension } from "@lexical/table";
import { configExtension, defineExtension } from "lexical";
import { EDITOR_HTML_CONFIG } from "#/editor/html-config";
import { EDITOR_NODES, EDITOR_THEME } from "#/editor/nodes/registry";

const HISTORY_MAX_DEPTH = 100;

export const DocumentContentExtension = defineExtension({
  name: "missal/DocumentContent",
  namespace: "missal-document",
  nodes: [...EDITOR_NODES],
  theme: EDITOR_THEME,
  html: EDITOR_HTML_CONFIG,
  onError: (error) => {
    console.error("Missal editor error", error);
  },
});

export const DocumentEditorExtension = defineExtension({
  name: "missal/DocumentEditor",
  dependencies: [
    DocumentContentExtension,
    RichTextExtension,
    ListExtension,
    configExtension(TableExtension, { hasHorizontalScroll: false }),
    LinkExtension,
    HorizontalRuleExtension,
    configExtension(HistoryExtension, { maxDepth: HISTORY_MAX_DEPTH }),
  ],
});

import { $generateNodesFromDOM } from "@lexical/html";
import {
  $getSelection,
  $insertNodes,
  $isElementNode,
  $isRangeSelection,
  type LexicalEditor,
  type LexicalNode,
} from "lexical";
import DOMPurify from "isomorphic-dompurify";
import { materializeClipboardPageBreaks } from "#/editor/import/page-breaks";
import { measureLineHeightRatio, normalizeWordHtml } from "#/editor/import/word-html";
import { $isPageBreakNode } from "#/editor/nodes/page-break-node";

export type ImportNotice = {
  readonly category: string;
  readonly message: string;
};

function sanitizeClipboardHtml(html: string) {
  return DOMPurify.sanitize(html, {
    ADD_ATTR: ["data-field", "data-field-reference", "data-page-break"],
    FORBID_TAGS: ["script", "iframe", "object", "embed", "form", "link", "meta"],
    FORBID_ATTR: ["srcset"],
    ALLOW_UNKNOWN_PROTOCOLS: false,
  });
}

export function prepareClipboardDom(html: string) {
  const sanitized = sanitizeClipboardHtml(html);
  const dom = new DOMParser().parseFromString(sanitized, "text/html");
  materializeClipboardPageBreaks(dom.body);
  return { dom, sanitized };
}

export function insertSanitizedHtml(editor: LexicalEditor, html: string) {
  const notices: ImportNotice[] = [];
  const { dom, sanitized } = prepareClipboardDom(
    normalizeWordHtml(html, { lineHeightRatio: measureLineHeightRatio }),
  );
  if (sanitized.includes("position:absolute") || sanitized.includes("float:")) {
    notices.push({
      category: "layout",
      message: "Floating layout was flattened to keep text.",
    });
  }

  for (const image of dom.querySelectorAll("img")) {
    const src = image.getAttribute("src") ?? "";
    if (!src.startsWith("data:image/")) {
      notices.push({
        category: "image",
        message: "Remote or local image paths were not loaded.",
      });
      image.remove();
    }
  }

  editor.update(
    () => {
      const nodes = $unwrapImportedPageBreaks($generateNodesFromDOM(editor, dom));
      const selection = $getSelection();
      if ($isRangeSelection(selection)) {
        selection.insertNodes(nodes);
      } else {
        $insertNodes(nodes);
      }
    },
    { discrete: true },
  );

  return notices;
}

function $unwrapImportedPageBreaks(nodes: LexicalNode[]): LexicalNode[] {
  const result: LexicalNode[] = [];
  for (const node of nodes) {
    if ($isElementNode(node)) {
      const children = node.getChildren();
      if (children.length > 0 && children.every($isPageBreakNode)) {
        result.push(...children);
        continue;
      }
    }

    result.push(node);
  }

  return result;
}

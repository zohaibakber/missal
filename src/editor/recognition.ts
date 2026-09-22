import {
  $getSelection,
  $isRangeSelection,
  COMMAND_PRIORITY_LOW,
  KEY_DOWN_COMMAND,
  TextNode,
  type LexicalEditor,
} from "lexical";
import { $createFieldNode } from "#/editor/nodes/field-node";
import type { PlaceholderIndex } from "#/lib/placeholder";
import { resolveFieldReference } from "#/lib/placeholder";
import { $copyTextStyleToField } from "#/editor/imported-style";

const TOKEN_PATTERN = /@(\d+|[A-Za-z][A-Za-z0-9_]*)@/;

export function registerFieldRecognition(editor: LexicalEditor, getIndex: () => PlaceholderIndex) {
  return editor.registerCommand(
    KEY_DOWN_COMMAND,
    (event) => {
      if (event.key !== "@" || event.isComposing) {
        return false;
      }

      let selected = "";
      editor.getEditorState().read(() => {
        const selection = $getSelection();
        if ($isRangeSelection(selection) && !selection.isCollapsed()) {
          selected = selection.getTextContent().trim();
        }
      });

      if (!selected) {
        return false;
      }

      editor.update(() => {
        const selection = $getSelection();
        if (!$isRangeSelection(selection) || selection.isCollapsed()) {
          return;
        }

        selection.insertNodes([$createFieldNode(resolveFieldReference(selected, getIndex()))]);
      });
      event.preventDefault();
      return true;
    },
    COMMAND_PRIORITY_LOW,
  );
}

export function registerCompletedTokenConversion(
  editor: LexicalEditor,
  getIndex: () => PlaceholderIndex,
) {
  return editor.registerNodeTransform(TextNode, (node) => {
    if (!node.isSimpleText() || editor.isComposing()) {
      return;
    }

    const match = TOKEN_PATTERN.exec(node.getTextContent());
    if (!match) {
      return;
    }

    // Transform every dirty text node, including earlier paragraphs and table cells in a paste.
    // Splitting preserves formatting and lets Lexical transform any remaining tokens in the tail.
    const start = match.index;
    const parts = node.splitText(start, start + match[0].length);
    const tokenNode = parts[start === 0 ? 0 : 1];
    const field = $createFieldNode(resolveFieldReference(match[1], getIndex()));
    $copyTextStyleToField(tokenNode, field);
    tokenNode.replace(field);
  });
}

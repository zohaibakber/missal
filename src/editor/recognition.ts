import {
  $createTextNode,
  $getSelection,
  $isRangeSelection,
  $isTextNode,
  COMMAND_PRIORITY_LOW,
  KEY_DOWN_COMMAND,
  type LexicalEditor,
} from "lexical";
import { $createFieldNode } from "#/editor/nodes/field-node";
import type { PlaceholderIndex } from "#/lib/placeholder";
import { isPlaceholderToken, resolveFieldReference } from "#/lib/placeholder";

const TOKEN_PATTERN = /@([^@\r\n]{1,120})@/;

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
  return editor.registerUpdateListener(({ dirtyLeaves, tags }) => {
    if (tags.has("historic") || dirtyLeaves.size === 0) {
      return;
    }

    editor.getEditorState().read(() => {
      const selection = $getSelection();
      if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
        return;
      }

      const anchor = selection.anchor.getNode();
      if (!$isTextNode(anchor)) {
        return;
      }

      const content = anchor.getTextContent();
      const match = TOKEN_PATTERN.exec(content);
      if (!match?.[1] || !isPlaceholderToken(match[1])) {
        return;
      }

      const start = match.index ?? 0;
      const token = match[1];
      queueMicrotask(() => {
        editor.update(() => {
          const current = $getSelection();
          if (!$isRangeSelection(current) || !current.isCollapsed()) {
            return;
          }

          const node = current.anchor.getNode();
          if (!$isTextNode(node)) {
            return;
          }

          const value = node.getTextContent();
          const found = TOKEN_PATTERN.exec(value);
          if (!found?.[1] || found[1] !== token || (found.index ?? 0) !== start) {
            return;
          }

          const end = start + found[0].length;
          node.setTextContent(value.slice(0, start));
          const field = $createFieldNode(resolveFieldReference(found[1], getIndex()));
          node.insertAfter(field);
          const rest = value.slice(end);
          if (rest) {
            field.insertAfter($createTextNode(rest));
          }
        });
      });
    });
  });
}

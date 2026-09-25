import {
  $getSelection,
  $isRangeSelection,
  COMMAND_PRIORITY_LOW,
  KEY_DOWN_COMMAND,
  TextNode,
  type LexicalEditor,
} from "lexical";
import { $createFieldNode } from "#/editor/nodes/field-node";
import { CatalogFieldReference, type FieldReference } from "#/lib/field";
import type { PlaceholderIndex } from "#/lib/placeholder";
import { resolveFieldReference, resolvePlaceholder } from "#/lib/placeholder";
import type { FieldMarkers } from "#/lib/settings";
import { $copyTextStyleToField } from "#/editor/imported-style";

/** Longest name looked for between markers; keeps a stray marker from swallowing a paragraph. */
const MAX_NAME_LENGTH = 80;

type TokenMatch = { start: number; end: number; reference: FieldReference };

/**
 * Finds the first `<open>name<close>` whose name is a known placeholder. Unknown names stay as
 * text so ordinary uses of the marker (an email address, say) are never turned into fields.
 */
export function findPlaceholderToken(
  text: string,
  markers: FieldMarkers,
  index: PlaceholderIndex,
): TokenMatch | undefined {
  for (let open = text.indexOf(markers.open); open !== -1;) {
    const nameStart = open + markers.open.length;
    const close = text.indexOf(markers.close, nameStart);
    if (close === -1) return undefined;
    const name = text.slice(nameStart, close);
    if (name.length <= MAX_NAME_LENGTH && !name.includes("\n")) {
      const placeholder = resolvePlaceholder(name, index);
      if (placeholder) {
        return {
          start: open,
          end: close + markers.close.length,
          reference: CatalogFieldReference.make({ id: placeholder.id }),
        };
      }
    }
    open = text.indexOf(markers.open, open + 1);
  }
  return undefined;
}

export function registerFieldRecognition(
  editor: LexicalEditor,
  getIndex: () => PlaceholderIndex,
  getMarkers: () => FieldMarkers,
) {
  return editor.registerCommand(
    KEY_DOWN_COMMAND,
    (event) => {
      if (event.key !== getMarkers().open[0] || event.isComposing) {
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
  getMarkers: () => FieldMarkers,
) {
  return editor.registerNodeTransform(TextNode, (node) => {
    if (!node.isSimpleText() || editor.isComposing()) {
      return;
    }

    const match = findPlaceholderToken(node.getTextContent(), getMarkers(), getIndex());
    if (!match) {
      return;
    }

    const parts = node.splitText(match.start, match.end);
    const tokenNode = parts[match.start === 0 ? 0 : 1];
    const field = $createFieldNode(match.reference);
    $copyTextStyleToField(tokenNode, field);
    tokenNode.replace(field);
  });
}

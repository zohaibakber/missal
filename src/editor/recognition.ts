import {
  $createTextNode,
  $getNearestNodeFromDOMNode,
  $getNodeByKey,
  $getSelection,
  $isRangeSelection,
  $isTextNode,
  COMMAND_PRIORITY_LOW,
  HISTORY_MERGE_TAG,
  KEY_DOWN_COMMAND,
  TextNode,
  type EditorState,
  type LexicalEditor,
} from "lexical";
import { $createFieldNode, $isFieldNode } from "#/editor/nodes/field-node";
import { fieldLabel, type FieldReference } from "#/lib/field";
import type { FieldIndex } from "#/lib/placeholder";
import { resolveField, resolveFieldReference } from "#/lib/placeholder";
import type { FieldMarkers } from "#/lib/settings";
import { $copyTextStyleToField, $getImportedStyle } from "#/editor/imported-style";

const MAX_NAME_LENGTH = 80;

type TokenMatch = { start: number; end: number; reference: FieldReference };

export function findPlaceholderToken(
  text: string,
  markers: FieldMarkers,
  index: FieldIndex,
  from = 0,
): TokenMatch | undefined {
  for (let open = text.indexOf(markers.open, from); open !== -1;) {
    const nameStart = open + markers.open.length;
    const close = text.indexOf(markers.close, nameStart);
    if (close === -1) return undefined;
    const name = text.slice(nameStart, close);
    if (name.length <= MAX_NAME_LENGTH && !name.includes("\n")) {
      const reference = resolveField(name, index);
      if (reference) return { start: open, end: close + markers.close.length, reference };
    }
    open = text.indexOf(markers.open, open + 1);
  }
  return undefined;
}

export function registerFieldRecognition(
  editor: LexicalEditor,
  getIndex: () => FieldIndex,
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

function $findConvertibleToken(
  node: TextNode,
  markers: FieldMarkers,
  index: FieldIndex,
): TokenMatch | undefined {
  const text = node.getTextContent();
  const selection = $getSelection();
  const carets = $isRangeSelection(selection)
    ? [selection.anchor, selection.focus].filter((point) => point.key === node.getKey())
    : [];
  let match = findPlaceholderToken(text, markers, index);
  while (match) {
    const { start, end } = match;
    if (!carets.some((point) => point.offset > start && point.offset < end)) return match;
    match = findPlaceholderToken(text, markers, index, end);
  }
  return undefined;
}

export function registerCompletedTokenConversion(
  editor: LexicalEditor,
  getIndex: () => FieldIndex,
  getMarkers: () => FieldMarkers,
) {
  const unregisterTransform = editor.registerNodeTransform(TextNode, (node) => {
    if (!node.isSimpleText() || editor.isComposing()) {
      return;
    }

    const match = $findConvertibleToken(node, getMarkers(), getIndex());
    if (!match) {
      return;
    }

    const parts = node.splitText(match.start, match.end);
    const tokenNode = parts[match.start === 0 ? 0 : 1];
    const field = $createFieldNode(match.reference);
    $copyTextStyleToField(tokenNode, field);
    tokenNode.replace(field);
  });

  const unregisterLeave = editor.registerUpdateListener(({ editorState, prevEditorState }) => {
    const previous = caretOf(prevEditorState);
    const current = caretOf(editorState);
    if (!previous || (previous.key === current?.key && previous.offset === current.offset)) {
      return;
    }
    const pending = editorState.read(() => {
      const node = $getNodeByKey(previous.key);
      return $isTextNode(node) && node.isSimpleText()
        ? $findConvertibleToken(node, getMarkers(), getIndex()) !== undefined
        : false;
    });
    if (pending) {
      editor.update(() => $getNodeByKey(previous.key)?.markDirty(), { tag: HISTORY_MERGE_TAG });
    }
  });

  return () => {
    unregisterTransform();
    unregisterLeave();
  };
}

function caretOf(state: EditorState) {
  return state.read(() => {
    const selection = $getSelection();
    return $isRangeSelection(selection)
      ? { key: selection.anchor.key, offset: selection.anchor.offset }
      : undefined;
  });
}

export function registerFieldReveal(
  editor: LexicalEditor,
  getIndex: () => FieldIndex,
  getMarkers: () => FieldMarkers,
) {
  let root: HTMLElement | null = null;

  const onDoubleClick = (event: MouseEvent) => {
    const host =
      event.target instanceof Element
        ? event.target.closest<HTMLElement>("[data-field='true']")
        : null;
    if (!host || !editor.isEditable()) return;
    event.preventDefault();
    editor.update(() => {
      const field = $getNearestNodeFromDOMNode(host);
      if (!$isFieldNode(field)) return;
      const name = fieldLabel(field.getReference(), getIndex());
      if (!name) return;
      const { open, close } = getMarkers();
      const text = $createTextNode(`${open}${name}${close}`).setStyle($getImportedStyle(field));
      field.replace(text);
      text.select(open.length, open.length + name.length);
    });
  };

  const unregisterRoot = editor.registerRootListener((next, previous) => {
    previous?.removeEventListener("dblclick", onDoubleClick);
    next?.addEventListener("dblclick", onDoubleClick);
    root = next;
  });

  return () => {
    unregisterRoot();
    root?.removeEventListener("dblclick", onDoubleClick);
  };
}

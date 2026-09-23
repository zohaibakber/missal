import {
  $getNodeByKey,
  $isTextNode,
  $isElementNode,
  type DOMConversion,
  type DOMConversionMap,
  type DOMExportOutputMap,
  type HTMLConfig,
  type LexicalEditor,
} from "lexical";
import { $isTableCellNode } from "@lexical/table";
import { EDITOR_NODES } from "#/editor/nodes/registry";
import { $applyImportedStyle, $setImportedStyle, retainedStyle } from "#/editor/imported-style";

function createImportMap(): DOMConversionMap {
  const converters = new Map<string, NonNullable<DOMConversionMap[string]>[]>();
  for (const node of EDITOR_NODES) {
    node.getType(); // Materialize static converters declared through Lexical's $config API.
    for (const [tag, convert] of Object.entries(node.importDOM?.() ?? {})) {
      if (!convert) continue;
      const entries = converters.get(tag) ?? [];
      entries.push(convert);
      converters.set(tag, entries);
    }
  }
  const result: DOMConversionMap = {};
  for (const [tag, entries] of converters) {
    result[tag] = (element) => {
      let selected: DOMConversion | null = null;
      for (const entry of entries) {
        const candidate = entry(element);
        if (candidate && (!selected || (candidate.priority ?? 0) >= (selected.priority ?? 0)))
          selected = candidate;
      }
      if (!selected) return null;
      const original = selected;
      return {
        priority: 4,
        conversion: (dom) => {
          const converted = original.conversion(dom);
          if (!converted) return converted;
          const nodes = Array.isArray(converted.node) ? converted.node : [converted.node];
          if (dom instanceof Text) {
            const textStyle = inheritedTextStyle(dom);
            for (const node of nodes) {
              if ($isTextNode(node) && textStyle)
                node.setStyle(retainedStyle(`${textStyle};${node.getStyle()}`, true));
            }
            return converted;
          }
          if (!(dom instanceof HTMLElement)) return converted;
          const style = retainedStyle(dom.style.cssText);
          for (const node of nodes) {
            if (!node || $isTextNode(node)) continue;
            if (style) $setImportedStyle(node, style);
            if ($isElementNode(node) && (dom.dir === "rtl" || dom.dir === "ltr")) {
              node.setDirection(dom.dir);
            }
          }
          return converted;
        },
      };
    };
  }
  return result;
}

/**
 * A run's text style is its ancestors' inline text styles, innermost last so it wins. Computed from
 * the DOM because Lexical's per-tag `forChild` hooks collapse nested elements with the same tag.
 */
function inheritedTextStyle(text: Text) {
  const styles: string[] = [];
  for (let element = text.parentElement; element; element = element.parentElement) {
    const style = retainedStyle(element.style.cssText, true);
    if (style) styles.unshift(style);
  }
  return retainedStyle(styles.join(";"), true);
}

const exports: DOMExportOutputMap = new Map(
  EDITOR_NODES.map((klass) => [
    klass,
    (editor, node) => {
      const output = node.exportDOM(editor);
      return {
        ...output,
        after: (element) => {
          const result = output.after ? output.after(element) : element;
          if (result instanceof HTMLElement) {
            // Lexical supplies arbitrary cell widths even when none were stored.
            if ($isTableCellNode(node) && node.getWidth() === undefined)
              result.style.removeProperty("width");
            $applyImportedStyle(node, result);
          }
          return result;
        },
      };
    },
  ]),
);

export const EDITOR_HTML_CONFIG: HTMLConfig = { import: createImportMap(), export: exports };

export function registerImportedStyleRendering(editor: LexicalEditor) {
  const unregister = EDITOR_NODES.map((klass) =>
    editor.registerMutationListener(klass, (mutations) => {
      editor.getEditorState().read(() => {
        for (const [key, mutation] of mutations) {
          if (mutation === "destroyed") continue;
          const node = $getNodeByKey(key);
          const element = editor.getElementByKey(key);
          if (node && element) $applyImportedStyle(node, element);
        }
      });
    }),
  );
  return () => {
    for (const dispose of unregister) dispose();
  };
}

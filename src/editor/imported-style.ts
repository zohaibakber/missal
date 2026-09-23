import { $getState, $setState, createState, type LexicalNode, type TextNode } from "lexical";

const TEXT_PROPERTIES = new Set([
  "font-family",
  "font-size",
  "font-weight",
  "font-style",
  "color",
  "background-color",
  "line-height",
  "letter-spacing",
  "white-space",
  "text-decoration",
  "vertical-align",
  "direction",
  "unicode-bidi",
]);
const LAYOUT_PROPERTIES = new Set([
  "text-align",
  "text-indent",
  "width",
  "min-width",
  "max-width",
  "height",
  "min-height",
  "margin",
  "margin-top",
  "margin-right",
  "margin-bottom",
  "margin-left",
  "margin-inline-start",
  "margin-inline-end",
  "padding",
  "padding-top",
  "padding-right",
  "padding-bottom",
  "padding-left",
  "padding-inline-start",
  "padding-inline-end",
  "border",
  "border-top",
  "border-right",
  "border-bottom",
  "border-left",
  "border-width",
  "border-style",
  "border-color",
  "border-collapse",
  "border-spacing",
  "table-layout",
  "break-inside",
  "break-before",
  "break-after",
  "page-break-inside",
  "page-break-before",
  "page-break-after",
  "orphans",
  "widows",
]);

// CSSOM expands shorthands (`border`, `margin`, `text-decoration`) into longhands when iterated.
const LONGHAND_FAMILIES = /^(?:border|margin|padding|text-decoration)-/;

function isRetained(property: string, textOnly: boolean) {
  if (TEXT_PROPERTIES.has(property)) return true;
  if (property.startsWith("text-decoration-")) return true;
  return !textOnly && (LAYOUT_PROPERTIES.has(property) || LONGHAND_FAMILIES.test(property));
}

// Sanitized once when a saved document is parsed and when set, so rendering can apply it directly.
const importedStyleState = createState("importedStyle", {
  parse: (value: unknown) => (typeof value === "string" ? retainedStyle(value) : ""),
});

export function retainedStyle(cssText: string, textOnly = false): string {
  if (!cssText) return "";
  const source = document.createElement("span").style;
  const result = document.createElement("span").style;
  source.cssText = cssText;
  for (const property of source) {
    const value = source.getPropertyValue(property);
    if (isRetained(property, textOnly) && !/url\s*\(|expression\s*\(|var\s*\(/i.test(value)) {
      result.setProperty(property, value);
    }
  }
  return result.cssText;
}

export function $setImportedStyle(node: LexicalNode, style: string) {
  $setState(node, importedStyleState, retainedStyle(style));
}

export function $applyImportedStyle(node: LexicalNode, element: HTMLElement) {
  const style = $getState(node, importedStyleState);
  if (!style) return;
  const source = document.createElement("span").style;
  source.cssText = style;
  for (const property of source)
    element.style.setProperty(property, source.getPropertyValue(property));
}

export function $copyTextStyleToField(source: TextNode, field: LexicalNode) {
  const style = document.createElement("span").style;
  style.cssText = source.getStyle();
  if (source.hasFormat("bold")) style.fontWeight = "700";
  if (source.hasFormat("italic")) style.fontStyle = "italic";
  const decorations = [
    source.hasFormat("underline") && "underline",
    source.hasFormat("strikethrough") && "line-through",
  ].filter(Boolean);
  if (decorations.length) style.textDecoration = decorations.join(" ");
  $setImportedStyle(field, style.cssText);
}

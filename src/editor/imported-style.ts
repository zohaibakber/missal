import { $isTableCellNode, $isTableNode, $isTableRowNode } from "@lexical/table";
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

const LONGHAND_FAMILIES = /^(?:border|margin|padding|text-decoration)-/;

const DEFAULT_VALUES: Record<string, ReadonlySet<string>> = {
  color: new Set(["rgb(0, 0, 0)", "black", "#000000", "#000", "windowtext"]),
  "background-color": new Set([
    "rgb(255, 255, 255)",
    "white",
    "#ffffff",
    "#fff",
    "transparent",
    "rgba(0, 0, 0, 0)",
  ]),
};

function isRetained(property: string, textOnly: boolean) {
  if (TEXT_PROPERTIES.has(property)) return true;
  if (property.startsWith("text-decoration-")) return true;
  return !textOnly && (LAYOUT_PROPERTIES.has(property) || LONGHAND_FAMILIES.test(property));
}

const importedStyleState = createState("importedStyle", {
  parse: (value: unknown) => (typeof value === "string" ? value : ""),
});

export function retainedStyle(cssText: string, textOnly = false): string {
  if (!cssText) return "";
  const source = document.createElement("span").style;
  const result = document.createElement("span").style;
  source.cssText = cssText;
  for (const property of source) {
    const value = source.getPropertyValue(property);
    if (
      isRetained(property, textOnly) &&
      !DEFAULT_VALUES[property]?.has(value.toLowerCase()) &&
      !/url\s*\(|expression\s*\(|var\s*\(/i.test(value)
    ) {
      result.setProperty(property, value);
    }
  }
  return result.cssText;
}

export function $getImportedStyle(node: LexicalNode): string {
  return $getState(node, importedStyleState) ?? "";
}

export function $setImportedStyle(node: LexicalNode, style: string) {
  $setState(node, importedStyleState, retainedStyle(style));
}

// Word tables often carry negative side margins and a width wider than the page.
const TABLE_CONTAINMENT = "margin-left: 0; margin-right: 0; max-width: 100%;";
const PX_PER_UNIT: Record<string, number> = {
  px: 1,
  pt: 96 / 72,
  pc: 16,
  in: 96,
  cm: 96 / 2.54,
  mm: 96 / 25.4,
};

function absoluteWidth(style: string): number | undefined {
  const declaration = document.createElement("span").style;
  declaration.cssText = style;
  const match = /^([\d.]+)(px|pt|pc|in|cm|mm)$/.exec(declaration.width);
  return match ? Number(match[1]) * PX_PER_UNIT[match[2]] : undefined;
}

// Fixed-layout tables never shrink below their summed column widths, so absolute
// cell widths become shares of the row and the table can fit the page.
function $relativeCellWidth(cell: LexicalNode, style: string): string | undefined {
  const width = absoluteWidth(style);
  const row = cell.getParent();
  if (width === undefined || !$isTableRowNode(row)) return undefined;
  let total = 0;
  for (const sibling of row.getChildren()) {
    total += absoluteWidth($getImportedStyle(sibling)) ?? 0;
  }
  return `width: ${((width / total) * 100).toFixed(3)}%;`;
}

export function $getRenderedImportedStyle(node: LexicalNode): string {
  const style = $getImportedStyle(node);
  if (!style) return style;
  if ($isTableNode(node)) return `${style} ${TABLE_CONTAINMENT}`;
  if ($isTableCellNode(node)) {
    const width = $relativeCellWidth(node, style);
    if (width) return `${style} ${width}`;
  }
  return style;
}

export function $applyImportedStyle(node: LexicalNode, element: HTMLElement) {
  const style = $getRenderedImportedStyle(node);
  if (style) element.style.cssText = `${element.style.cssText};${style}`;
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

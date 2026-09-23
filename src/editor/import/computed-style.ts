const TEXT_STYLE = [
  "font-family",
  "font-size",
  "font-weight",
  "font-style",
  "color",
  "background-color",
  "line-height",
  "white-space",
  "text-align",
  "text-indent",
  "text-decoration",
  "vertical-align",
  "direction",
  "unicode-bidi",
];
const BOX_STYLE = [
  "margin-top",
  "margin-right",
  "margin-bottom",
  "margin-left",
  "padding-top",
  "padding-right",
  "padding-bottom",
  "padding-left",
  "border-top",
  "border-right",
  "border-bottom",
  "border-left",
  "border-collapse",
  "border-spacing",
  "table-layout",
  "break-inside",
  "break-before",
  "break-after",
  "min-height",
];

export function collectStyles(
  element: HTMLElement,
  computed: CSSStyleDeclaration,
  keepNormal: ReadonlySet<string> = new Set(),
): string {
  const result = element.ownerDocument.createElement("span").style;
  for (const property of [...TEXT_STYLE, ...BOX_STYLE]) {
    const value = computed.getPropertyValue(property);
    if (value && value !== "auto" && (value !== "normal" || keepNormal.has(property)))
      result.setProperty(property, value);
  }
  // Paragraph and cell heights are content-dependent. Keep only authored dimensions.
  for (const property of ["width", "height", "min-width", "max-width"]) {
    const value = element.style.getPropertyValue(property);
    if (value && value !== "auto") result.setProperty(property, value);
  }
  return result.cssText;
}

export function cssLengthMm(value: string, fallback: number): number {
  const parsed = /^(\d+(?:\.\d+)?)(px|pt|mm|cm|in)$/.exec(value);
  if (!parsed) return fallback;
  const number = Number(parsed[1]);
  const unit = parsed[2];
  const millimeters =
    unit === "px"
      ? (number * 25.4) / 96
      : unit === "pt"
        ? (number * 25.4) / 72
        : unit === "cm"
          ? number * 10
          : unit === "in"
            ? number * 25.4
            : number;
  return Math.round(millimeters * 1000) / 1000;
}

import { generate, parse, walk, type CssNode, type Selector } from "css-tree";
import { cssLengthMm } from "#/editor/import/computed-style";
import { PageLayout } from "#/lib/document-format";
import { URDU_FONT_FAMILY, URDU_FONT_URL } from "#/lib/output";

const WORD_MARKERS =
  /urn:schemas-microsoft-com:office:(?:word|office)|<meta[^>]+content=["']?Microsoft Word|class=["']?Mso/i;
const ARABIC_SCRIPT = /[؀-ۿݐ-ݿࢠ-ࣿﭐ-﷿ﹰ-﻿]/;
// Word stores complex-script (Urdu/Arabic) run formatting separately; browsers ignore these properties.
const COMPLEX_SCRIPT_PROPERTIES = [
  ["mso-bidi-font-size", "font-size"],
  ["mso-bidi-font-family", "font-family"],
  ["mso-bidi-font-weight", "font-weight"],
  ["mso-bidi-font-style", "font-style"],
] as const;
const BLOCKS = "p, h1, h2, h3, h4, h5, h6, li, td, th";

type NormalizeOptions = {
  /** Font's natural line height divided by its size; Word's % spacing is relative to this. */
  lineHeightRatio?: (fontFamily: string) => number | undefined;
};

export function isWordHtml(html: string) {
  return WORD_MARKERS.test(html);
}

/**
 * Makes Word clipboard HTML self-contained: stylesheet rules (including Word's heading and Normal
 * styles) become inline styles, and Urdu runs use Word's complex-script size and font.
 */
export function normalizeWordHtml(
  html: string,
  { lineHeightRatio }: NormalizeOptions = {},
): string {
  if (!isWordHtml(html)) return html;

  const doc = new DOMParser().parseFromString(html, "text/html");
  inlineStylesheets(doc);

  // Resolve everything before writing, because writing through CSSOM drops the mso-* declarations.
  const spacing = [...doc.body.querySelectorAll<HTMLElement>("[style]")].flatMap((element) => {
    const percent = /^(\d+(?:\.\d+)?)%$/.exec(element.style.lineHeight)?.[1];
    const family = percent ? effectiveFontFamily(element) : undefined;
    const ratio = family ? lineHeightRatio?.(family) : undefined;
    return percent && ratio ? [{ element, lineHeight: (Number(percent) / 100) * ratio }] : [];
  });
  const blocks = [...doc.body.querySelectorAll<HTMLElement>(BLOCKS)];
  const singleSpaced = blocks
    .filter((block) => !block.style.lineHeight || block.style.lineHeight === "normal")
    .map((block) => {
      const family = effectiveFontFamily(block);
      return { block, singleLine: family ? lineHeightRatio?.(family) : undefined };
    });
  const autoSpaced = blocks.flatMap((block) =>
    (["top", "bottom"] as const).flatMap((side) =>
      declaration(block.getAttribute("style") ?? "", `mso-margin-${side}-alt`) === "auto"
        ? [{ block, side }]
        : [],
    ),
  );
  const runs = [...doc.body.querySelectorAll<HTMLElement>("*")].flatMap((element) =>
    [...element.childNodes]
      .filter(
        (node): node is Text =>
          node.nodeType === Node.TEXT_NODE && ARABIC_SCRIPT.test(node.textContent ?? ""),
      )
      .map((text) => ({ text, style: complexScriptStyle(element) })),
  );
  for (const { text, style } of runs) {
    if (!style) continue;
    const span = doc.createElement("span");
    span.setAttribute("style", style);
    text.replaceWith(span);
    span.append(text);
  }

  // Word's "multiple" spacing scales the font's single line, not its size as CSS percentages do.
  for (const { element, lineHeight } of spacing) {
    element.style.lineHeight = String(Math.round(lineHeight * 1000) / 1000);
  }

  // Word's "single" spacing is one font line; don't let the page's default line height apply.
  for (const { block, singleLine } of singleSpaced) {
    block.style.lineHeight = singleLine ? String(Math.round(singleLine * 1000) / 1000) : "normal";
  }

  // Word's "Auto" paragraph spacing is 14pt and has no CSS equivalent.
  for (const { block, side } of autoSpaced) block.style.setProperty(`margin-${side}`, "14pt");

  return doc.body.innerHTML;
}

/** Reads the first section's paper size and margins from Word's `@page` rule. */
export function readWordPageLayout(html: string): PageLayout | undefined {
  if (!isWordHtml(html)) return undefined;
  const rule = /@page\s+WordSection1\s*\{([^}]*)\}/i.exec(html)?.[1];
  if (!rule) return undefined;
  const size = declaration(rule, "size")?.split(/\s+/);
  const margins = declaration(rule, "margin")?.split(/\s+/) ?? [];
  const [top, right = top, bottom = top, left = right] = margins;
  if (!size?.[0] || !size[1]) return undefined;
  return new PageLayout({
    widthMm: cssLengthMm(size[0], 210),
    heightMm: cssLengthMm(size[1], 297),
    marginTopMm: cssLengthMm(top ?? "", 25.4),
    marginRightMm: cssLengthMm(right ?? "", 25.4),
    marginBottomMm: cssLengthMm(bottom ?? "", 25.4),
    marginLeftMm: cssLengthMm(left ?? "", 25.4),
  });
}

type StyleRule = {
  selector: string;
  specificity: readonly [number, number, number];
  order: number;
  declarations: string;
};

/**
 * Applies Word's `<style>` rules as inline styles with CSS precedence (specificity, then source
 * order, then the element's own inline style). Declarations are copied as text, so Word-only
 * properties such as `mso-bidi-font-size` survive for the complex-script pass.
 */
function inlineStylesheets(doc: Document) {
  const rules: StyleRule[] = [];
  for (const style of doc.querySelectorAll("style")) {
    const ast = parse(style.textContent ?? "", { parseValue: false, onParseError: () => {} });
    walk(ast, {
      visit: "Rule",
      enter(rule) {
        // Skip rules nested in @media/@page and friends; they don't describe the pasted content.
        if (this.atrule || rule.prelude.type !== "SelectorList") return;
        const declarations = generate(rule.block).replace(/^\{|\}$/g, "");
        for (const selector of rule.prelude.children) {
          if (selector.type !== "Selector") continue;
          rules.push({
            selector: generate(selector),
            specificity: specificityOf(selector),
            order: rules.length,
            declarations,
          });
        }
      },
    });
    style.remove();
  }
  rules.sort(
    (left, right) =>
      left.specificity[0] - right.specificity[0] ||
      left.specificity[1] - right.specificity[1] ||
      left.specificity[2] - right.specificity[2] ||
      left.order - right.order,
  );
  for (const element of doc.body.querySelectorAll<HTMLElement>("*")) {
    const matched = rules.filter((rule) => matchesSafely(element, rule.selector));
    if (!matched.length) continue;
    const own = element.getAttribute("style") ?? "";
    element.setAttribute("style", [...matched.map((rule) => rule.declarations), own].join(";"));
  }
}

function matchesSafely(element: Element, selector: string) {
  try {
    return element.matches(selector);
  } catch {
    return false; // Word emits a few selectors browsers can't parse (e.g. `@list` levels).
  }
}

function specificityOf(selector: Selector): [number, number, number] {
  const counts: [number, number, number] = [0, 0, 0];
  walk(selector, (node: CssNode) => {
    if (node.type === "IdSelector") counts[0] += 1;
    else if (
      node.type === "ClassSelector" ||
      node.type === "AttributeSelector" ||
      node.type === "PseudoClassSelector"
    )
      counts[1] += 1;
    else if (node.type === "TypeSelector" && node.name !== "*") counts[2] += 1;
    else if (node.type === "PseudoElementSelector") counts[2] += 1;
  });
  return counts;
}

const lineHeightRatios = new Map<string, number | undefined>();
/** Fonts shipped with the app; Word's single line is their Windows ascent + descent. */
const BUNDLED_FONTS = [{ family: URDU_FONT_FAMILY, url: URDU_FONT_URL }];

function primaryFamily(fontFamily: string) {
  return (fontFamily.split(",")[0] ?? "")
    .trim()
    .replace(/^["']|["']$/g, "")
    .toLowerCase();
}

/**
 * A font's single-line height divided by its size, as Word computes it. Uses the font file's
 * Windows metrics when known, otherwise the browser's `normal` line height for the loaded font.
 */
export function measureLineHeightRatio(fontFamily: string): number | undefined {
  const key = primaryFamily(fontFamily);
  if (lineHeightRatios.has(key)) return lineHeightRatios.get(key);
  const probe = document.createElement("div");
  probe.style.cssText =
    "position:absolute;visibility:hidden;inset-inline-start:-9999px;font-size:100px;line-height:normal;white-space:nowrap";
  probe.style.fontFamily = fontFamily;
  probe.textContent = "Hxgجناب";
  document.body.append(probe);
  const height = probe.getBoundingClientRect().height;
  probe.remove();
  const ratio = height > 0 ? height / 100 : undefined;
  // A fallback font's metrics would be wrong for good; only remember real measurements.
  if (document.fonts.check(`100px ${fontFamily}`)) lineHeightRatios.set(key, ratio);
  return ratio;
}

/** Loads the fonts the pasted HTML uses and reads bundled fonts' Windows line metrics. */
export async function prepareFontsFor(html: string) {
  const faces = [...document.fonts].filter(
    (face) => face.status === "unloaded" && html.includes(face.family.replace(/["']/g, "")),
  );
  const bundled = BUNDLED_FONTS.filter(
    (font) => html.includes(font.family) && !lineHeightRatios.has(primaryFamily(font.family)),
  );
  await Promise.all([
    ...faces.map((face) => face.load().catch(() => undefined)),
    ...bundled.map(async (font) => {
      try {
        const ratio = windowsLineRatio(await (await fetch(font.url)).arrayBuffer());
        if (ratio) lineHeightRatios.set(primaryFamily(font.family), ratio);
      } catch {
        // Fall back to the browser's measurement.
      }
    }),
  ]);
}

/** (usWinAscent + usWinDescent) / unitsPerEm from a TrueType/OpenType file. */
export function windowsLineRatio(buffer: ArrayBuffer): number | undefined {
  const view = new DataView(buffer);
  const tables = new Map<string, number>();
  for (let index = 0; index < view.getUint16(4); index++) {
    const record = 12 + index * 16;
    const tag = String.fromCharCode(
      ...[0, 1, 2, 3].map((offset) => view.getUint8(record + offset)),
    );
    tables.set(tag, view.getUint32(record + 8));
  }
  const head = tables.get("head");
  const os2 = tables.get("OS/2");
  if (head === undefined || os2 === undefined) return undefined;
  const unitsPerEm = view.getUint16(head + 18);
  const ascent = view.getUint16(os2 + 74);
  const descent = view.getUint16(os2 + 76);
  return unitsPerEm ? (ascent + descent) / unitsPerEm : undefined;
}

/** Urdu text renders in Word's complex-script font; everything else in the Latin font. */
function effectiveFontFamily(element: HTMLElement) {
  const complex = ARABIC_SCRIPT.test(element.textContent ?? "");
  return (
    (complex ? inheritedDeclaration(element, "mso-bidi-font-family") : undefined) ??
    inheritedDeclaration(element, "font-family")
  );
}

function complexScriptStyle(element: HTMLElement) {
  const style = COMPLEX_SCRIPT_PROPERTIES.flatMap(([complex, standard]) => {
    const value = inheritedDeclaration(element, complex);
    return value ? [`${standard}:${value}`] : [];
  });
  return style.join(";");
}

function inheritedDeclaration(element: HTMLElement, property: string): string | undefined {
  for (let current: HTMLElement | null = element; current; current = current.parentElement) {
    const value = declaration(current.getAttribute("style") ?? "", property);
    if (value) return value;
  }
  return undefined;
}

/** The last declaration of `property` wins, as in CSS. */
function declaration(style: string, property: string) {
  const escaped = property.replace(/[-/\\^$*+?.()|[\]{}]/g, "\\$&");
  const matches = [...style.matchAll(new RegExp(`(?:^|[;{\\s])${escaped}\\s*:\\s*([^;}]+)`, "gi"))];
  return matches
    .at(-1)?.[1]
    ?.trim()
    .replace(/\s*!important$/i, "");
}

import { PageLayout } from "#/lib/document-format";
import { collectStyles, cssLengthMm } from "#/editor/import/computed-style";
import { createPageBreakMarker } from "#/editor/import/page-breaks";
import { wordLineRatio } from "#/editor/import/word-html";
import { findPlaceholderToken } from "#/editor/recognition";
import type { PlaceholderIndex } from "#/lib/placeholder";
import type { FieldMarkers } from "#/lib/settings";

const WORD_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
export async function importDocx(
  file: File,
  fields: { markers: FieldMarkers; index: PlaceholderIndex },
): Promise<{
  html: string;
  pageLayout: PageLayout;
  notices: string[];
}> {
  const [{ renderAsync }, { default: JSZip }] = await Promise.all([
    import("docx-preview"),
    import("jszip"),
  ]);
  const archive = await JSZip.loadAsync(await file.arrayBuffer());
  for (const entry of Object.values(archive.files)) {
    if (
      /^word\/(?:document|styles|header\d+|footer\d+|footnotes|endnotes)\.xml$/.test(entry.name)
    ) {
      const xml = normalizeComplexScriptFormatting(
        mergeFieldsToTokens(await entry.async("string"), fields.markers),
      );
      archive.file(entry.name, joinSplitFieldTokens(xml, fields.markers, fields.index));
    }
  }

  const frame = document.createElement("iframe");
  frame.setAttribute("aria-hidden", "true");
  frame.tabIndex = -1;
  frame.style.cssText =
    "position:fixed;left:-100000px;top:0;width:1200px;height:1000px;visibility:hidden";
  document.body.append(frame);
  try {
    const doc = frame.contentDocument;
    const view = frame.contentWindow;
    if (!doc || !view) throw new Error("Could not create a document import window.");
    const policy = doc.createElement("meta");
    policy.httpEquiv = "Content-Security-Policy";
    policy.content = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:";
    doc.head.append(policy);
    const styleHost = doc.createElement("div");
    doc.head.append(styleHost);
    await renderAsync(await archive.generateAsync({ type: "arraybuffer" }), doc.body, styleHost, {
      inWrapper: false,
      breakPages: true,
      ignoreLastRenderedPageBreak: true,
      useBase64URL: true,
      renderAltChunks: false,
      renderComments: false,
    });
    markAutoLineHeights(doc);
    const pages = [...doc.querySelectorAll<HTMLElement>("section.docx")];
    const firstPage = pages[0];
    if (!firstPage) throw new Error("This DOCX has no readable document pages.");
    const pageLayout = readPageLayout(view.getComputedStyle(firstPage));
    const notices: string[] = [];
    if (
      pages.some((page) => !samePageLayout(pageLayout, readPageLayout(view.getComputedStyle(page))))
    ) {
      notices.push(
        "This document uses different page sizes or margins. Printing uses the first page's settings.",
      );
    }
    if (doc.querySelector("header, footer")) {
      notices.push("Headers and footers were imported as editable content on each explicit page.");
    }
    if (doc.querySelector("svg, math")) {
      notices.push("Some drawings or equations may need to be checked after import.");
    }
    const output = doc.createElement("div");
    for (const [index, page] of pages.entries()) {
      if (index > 0) output.append(createPageBreakMarker(doc));
      // Snapshot before writing styles: changing a parent must not change its descendants' measurements.
      const styled = [...page.querySelectorAll<HTMLElement>("*")].map((element) => {
        const computed = view.getComputedStyle(element);
        return { element, styles: withWordLineHeight(collectStyles(element, computed), computed) };
      });
      for (const { element, styles } of styled) {
        element.removeAttribute("class");
        element.setAttribute("style", styles);
        const direction = element.style.direction;
        if (direction === "rtl" || direction === "ltr") element.dir = direction;
      }
      for (const image of page.querySelectorAll("img")) {
        if (!(image.getAttribute("src") ?? "").startsWith("data:image/")) {
          image.remove();
          notices.push("An image could not be embedded and was omitted.");
        }
      }
      // Snapshot first: appending a child moves it out of the live `children` collection.
      for (const child of Array.from(page.children)) {
        if (
          child.tagName === "ARTICLE" ||
          child.tagName === "HEADER" ||
          child.tagName === "FOOTER"
        ) {
          output.append(...child.childNodes);
        } else {
          output.append(child);
        }
      }
    }
    return { html: output.innerHTML, pageLayout, notices: [...new Set(notices)] };
  } finally {
    frame.remove();
  }
}

const AUTO_LINE = "--missal-auto-line";

/**
 * docx-preview writes Word's "auto" line spacing as a multiple of the font size, but Word
 * multiplies the font's own single-line height, which for Nastaleeq is far taller. Tag those
 * multiples so they can be rescaled per font; any other line height clears the tag.
 */
function markAutoLineHeights(doc: Document) {
  const mark = (css: string) =>
    css.replace(/line-height\s*:\s*([^;}"]+)/g, (declaration, value: string) => {
      const multiple = value.trim();
      return /^\d+(?:\.\d+)?$/.test(multiple)
        ? `${declaration}; ${AUTO_LINE}: ${multiple}`
        : `${declaration}; ${AUTO_LINE}: initial`;
    });
  for (const sheet of doc.querySelectorAll("style"))
    sheet.textContent = mark(sheet.textContent ?? "");
  for (const element of doc.querySelectorAll<HTMLElement>("[style*='line-height']")) {
    element.setAttribute("style", mark(element.getAttribute("style") ?? ""));
  }
}

export function withWordLineHeight(styles: string, computed: CSSStyleDeclaration) {
  const auto = computed.getPropertyValue(AUTO_LINE).trim();
  const multiple = auto ? Number(auto) : computed.lineHeight === "normal" ? 1 : undefined;
  const ratio = multiple ? wordLineRatio(computed.fontFamily) : undefined;
  const fontSize = Number.parseFloat(computed.fontSize);
  if (!multiple || !ratio || !fontSize) return styles;
  const style = new DOMParser().parseFromString("<p></p>", "text/html").body.style;
  style.cssText = styles;
  style.setProperty("line-height", `${Math.round(multiple * ratio * fontSize * 100) / 100}px`);
  return style.cssText;
}

function readPageLayout(style: CSSStyleDeclaration): PageLayout {
  return new PageLayout({
    widthMm: cssLengthMm(style.width, 210),
    heightMm: cssLengthMm(style.minHeight, 297),
    marginTopMm: cssLengthMm(style.paddingTop, 25.4),
    marginRightMm: cssLengthMm(style.paddingRight, 25.4),
    marginBottomMm: cssLengthMm(style.paddingBottom, 25.4),
    marginLeftMm: cssLengthMm(style.paddingLeft, 25.4),
  });
}

function samePageLayout(left: PageLayout, right: PageLayout): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

export function normalizeComplexScriptFormatting(xml: string): string {
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  if (doc.querySelector("parsererror")) throw new Error("This DOCX contains invalid document XML.");
  // docx-preview drops text inside bidi embeddings; the runs carry their own direction anyway.
  for (const name of ["dir", "bdo"]) {
    for (const embedding of Array.from(doc.getElementsByTagNameNS(WORD_NS, name))) {
      embedding.replaceWith(...embedding.childNodes);
    }
  }
  // docx-preview ignores right-to-left table layout and reads indents from the wrong attribute.
  for (const table of doc.getElementsByTagNameNS(WORD_NS, "tblPr")) {
    const children = [...table.children];
    const visual = children.find((child) => child.localName === "bidiVisual");
    const rightToLeft =
      visual && !["0", "false", "off"].includes(visual.getAttributeNS(WORD_NS, "val") ?? "");
    if (rightToLeft && !children.some((child) => child.localName === "bidi")) {
      table.append(doc.createElementNS(WORD_NS, "w:bidi"));
    }
    const indent = children.find((child) => child.localName === "tblInd");
    const width = indent?.getAttributeNS(WORD_NS, "w");
    const type = indent?.getAttributeNS(WORD_NS, "type") ?? "dxa";
    if (indent && width && type === "dxa") indent.setAttributeNS(WORD_NS, "w:start", width);
  }
  for (const properties of doc.getElementsByTagNameNS(WORD_NS, "rPr")) {
    const children = [...properties.children];
    const rtl = children.find((child) => child.localName === "rtl" || child.localName === "cs");
    const rtlValue = rtl?.getAttributeNS(WORD_NS, "val");
    if (!rtl || rtlValue === "0" || rtlValue === "false" || rtlValue === "off") continue;
    for (const [complex, standard] of [
      ["szCs", "sz"],
      ["bCs", "b"],
      ["iCs", "i"],
    ]) {
      if (!complex || !standard) continue;
      const source = children.find((child) => child.localName === complex);
      if (!source) continue;
      let target = children.find((child) => child.localName === standard);
      if (!target) {
        target = doc.createElementNS(WORD_NS, `w:${standard}`);
        properties.append(target);
      }
      const value = source.getAttributeNS(WORD_NS, "val");
      if (value === null) target.removeAttributeNS(WORD_NS, "val");
      else target.setAttributeNS(WORD_NS, "w:val", value);
    }
    const fonts = children.find((child) => child.localName === "rFonts");
    const complexFont = fonts?.getAttributeNS(WORD_NS, "cs");
    if (fonts && complexFont) {
      fonts.setAttributeNS(WORD_NS, "w:ascii", complexFont);
      fonts.setAttributeNS(WORD_NS, "w:hAnsi", complexFont);
    }
  }
  return new XMLSerializer().serializeToString(doc);
}

const MERGE_FIELD = /^\s*MERGEFIELD\s+(?:"([^"]+)"|(\S+))/i;

type OpenField = { instruction: string; inResult: boolean; runs: Element[]; result: Element[] };

/**
 * Mail-merge templates hold `MERGEFIELD name` fields whose shown text is a sample record's value.
 * Replaces each with `<open>name<close>` in the sample's formatting so it imports as a field, not as
 * someone's data.
 */
export function mergeFieldsToTokens(xml: string, markers: FieldMarkers): string {
  if (!xml.includes("MERGEFIELD")) return xml;
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  if (doc.querySelector("parsererror")) throw new Error("This DOCX contains invalid document XML.");
  const token = (name: string) => `${markers.open}${name}${markers.close}`;
  const child = (parent: Element, name: string) =>
    [...parent.children].find((node) => node.namespaceURI === WORD_NS && node.localName === name);
  const tokenRun = (template: Element | undefined, text: string) => {
    const run = doc.createElementNS(WORD_NS, "w:r");
    const properties = template && child(template, "rPr");
    if (properties) run.append(properties.cloneNode(true));
    const content = doc.createElementNS(WORD_NS, "w:t");
    content.setAttribute("xml:space", "preserve");
    content.textContent = text;
    run.append(content);
    return run;
  };

  // Snapshot the live collections: replacing fields removes elements from them.
  for (const simple of Array.from(doc.getElementsByTagNameNS(WORD_NS, "fldSimple"))) {
    const match = MERGE_FIELD.exec(simple.getAttributeNS(WORD_NS, "instr") ?? "");
    const name = match?.[1] ?? match?.[2];
    if (!name) continue;
    const runs = [...simple.getElementsByTagNameNS(WORD_NS, "r")];
    simple.replaceWith(tokenRun(runs[0], token(name)));
  }

  // Complex fields: begin, instruction runs, separate, result runs, end. Fields may nest.
  const open: OpenField[] = [];
  for (const run of Array.from(doc.getElementsByTagNameNS(WORD_NS, "r"))) {
    const marker = child(run, "fldChar")?.getAttributeNS(WORD_NS, "fldCharType");
    const field = open.at(-1);
    if (marker === "begin") {
      open.push({ instruction: "", inResult: false, runs: [run], result: [] });
      continue;
    }
    if (!field) continue;
    field.runs.push(run);
    if (marker === "separate") {
      field.inResult = true;
    } else if (marker === "end") {
      open.pop();
      const match = MERGE_FIELD.exec(field.instruction);
      const name = match?.[1] ?? match?.[2];
      const parent = open.at(-1);
      const paragraph = closestParagraph(run);
      // Leave fields that span paragraphs or sit inside another field's instruction.
      if (
        !name ||
        (parent && !parent.inResult) ||
        field.runs.some((item) => closestParagraph(item) !== paragraph)
      ) {
        parent?.runs.push(...field.runs);
        continue;
      }
      const replacement = tokenRun(
        field.result.find((item) => child(item, "t")) ?? field.runs[0],
        token(name),
      );
      run.after(replacement);
      for (const item of field.runs) item.remove();
      parent?.runs.push(replacement);
      if (parent?.inResult) parent.result.push(replacement);
    } else if (field.inResult) {
      field.result.push(run);
    } else {
      for (const text of run.getElementsByTagNameNS(WORD_NS, "instrText")) {
        field.instruction += text.textContent ?? "";
      }
    }
  }
  return new XMLSerializer().serializeToString(doc);
}

/**
 * Word splits text into runs freely, e.g. `«`, `Date`, `_`, `FIR`, `»` with different direction
 * flags. Moves each known field token into the run holding its opening marker so it imports as
 * one piece of text and becomes a field.
 */
export function joinSplitFieldTokens(
  xml: string,
  markers: FieldMarkers,
  index: PlaceholderIndex,
): string {
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  if (doc.querySelector("parsererror")) throw new Error("This DOCX contains invalid document XML.");
  let changed = false;
  for (const paragraph of doc.getElementsByTagNameNS(WORD_NS, "p")) {
    // Text boxes nest paragraphs inside runs; each paragraph only handles its own text.
    const texts = [...paragraph.getElementsByTagNameNS(WORD_NS, "t")].filter(
      (text) => closestParagraph(text) === paragraph,
    );
    if (texts.length < 2) continue;
    const starts: number[] = [];
    let content = "";
    for (const text of texts) {
      starts.push(content.length);
      content += text.textContent ?? "";
    }
    const pieceAt = (offset: number) => starts.findLastIndex((start) => start <= offset);
    for (
      let match = findPlaceholderToken(content, markers, index);
      match;
      match = findPlaceholderToken(content, markers, index, match.end)
    ) {
      const first = pieceAt(match.start);
      const last = pieceAt(match.end - 1);
      if (first === last) continue;
      for (let piece = first; piece <= last; piece += 1) {
        const text = texts[piece];
        const start = starts[piece];
        if (!text || start === undefined) continue;
        const value = text.textContent ?? "";
        const from = Math.max(match.start - start, 0);
        const to = Math.min(match.end - start, value.length);
        const token = piece === first ? content.slice(match.start, match.end) : "";
        text.textContent = value.slice(0, from) + token + value.slice(to);
        text.setAttribute("xml:space", "preserve");
      }
      // The paragraph's text is unchanged; only the pieces after the first now start at the end.
      for (let piece = first + 1; piece <= last; piece += 1) starts[piece] = match.end;
      changed = true;
    }
  }
  return changed ? new XMLSerializer().serializeToString(doc) : xml;
}

function closestParagraph(node: Node): Element | null {
  for (let parent = node.parentNode; parent; parent = parent.parentNode) {
    if (parent instanceof Element && parent.namespaceURI === WORD_NS && parent.localName === "p") {
      return parent;
    }
  }
  return null;
}

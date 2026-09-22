import { PageLayout } from "#/lib/document-format";
import { collectStyles, cssLengthMm } from "#/editor/import/computed-style";
import { createPageBreakMarker } from "#/editor/import/page-breaks";

const WORD_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
/** Read the original archive without modifying the user's file. */
export async function importDocx(file: File): Promise<{
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
      archive.file(entry.name, normalizeComplexScriptFormatting(await entry.async("string")));
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
      const styled = [...page.querySelectorAll<HTMLElement>("*")].map((element) => ({
        element,
        styles: collectStyles(element, view.getComputedStyle(element)),
      }));
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

/** docx-preview reads Latin run properties; mirror the complex-script properties for RTL runs. */
export function normalizeComplexScriptFormatting(xml: string): string {
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  if (doc.querySelector("parsererror")) throw new Error("This DOCX contains invalid document XML.");
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

const PAGE_BREAK_VALUES = new Set(["always", "page", "left", "right", "recto", "verso"]);
const WORD_SECTION_CLASS = /^(?:Word)?Section(\d+)$/i;
const STYLE_BREAK_BEFORE =
  /(?:^|;)\s*(?:page-break-before|break-before)\s*:\s*(?:always|page|left|right|recto|verso)\b/i;
const STYLE_BREAK_AFTER =
  /(?:^|;)\s*(?:page-break-after|break-after)\s*:\s*(?:always|page|left|right|recto|verso)\b/i;
const STYLE_MSO_BREAK = /(?:^|;)\s*mso-break-type\s*:\s*(?:section-break|page-break)\b/i;

export function createPageBreakMarker(doc: Document): HTMLDivElement {
  const divider = doc.createElement("div");
  divider.className = "missal-page-break";
  divider.dataset.pageBreak = "true";
  return divider;
}

export function isPageBreakMarker(node: Node | null | undefined): node is HTMLElement {
  return node instanceof HTMLElement && node.dataset.pageBreak === "true";
}

export function htmlElementIsPageBreakImport(element: HTMLElement): boolean {
  if (element.dataset.pageBreak === "true" || element.classList.contains("missal-page-break")) {
    return true;
  }

  if (element.tagName !== "BR" && element.tagName !== "HR") {
    return false;
  }

  return (
    hasCssPageBreak(element, "before") ||
    hasCssPageBreak(element, "after") ||
    hasMsoPageBreak(element)
  );
}

export function materializeClipboardPageBreaks(root: ParentNode): void {
  const doc = ownerDocument(root);
  if (!doc) {
    return;
  }

  for (const element of snapshotElements(root)) {
    if (!element.isConnected || isPageBreakMarker(element)) {
      continue;
    }

    const msoBreak = hasMsoPageBreak(element);
    const breakBefore = hasCssPageBreak(element, "before") || msoBreak;
    const breakAfter = hasCssPageBreak(element, "after");
    const wordSection = wordSectionNumber(element);

    if (shouldReplaceElementWithMarker(element, breakBefore || breakAfter || msoBreak)) {
      replaceWithMarker(element, doc);
      continue;
    }

    if (breakBefore) {
      insertMarkerBefore(element, doc);
      stripCssPageBreak(element, "before");
    }

    if (breakAfter) {
      insertMarkerAfter(element, doc);
      stripCssPageBreak(element, "after");
    }

    if (wordSection !== null && wordSection > 1) {
      insertMarkerBefore(element, doc);
    }
  }

  liftPageBreakMarkers(root);
  unwrapWordSectionWrappers(root);
  collapseAdjacentPageBreaks(root);
  dropEdgeOnlyPageBreaks(root);
}

function ownerDocument(root: ParentNode): Document | null {
  if (root instanceof Document) {
    return root;
  }

  return root.ownerDocument;
}

function snapshotElements(root: ParentNode): HTMLElement[] {
  return Array.from(root.querySelectorAll("*")).filter(
    (node): node is HTMLElement => node instanceof HTMLElement,
  );
}

function hasCssPageBreak(element: HTMLElement, edge: "before" | "after"): boolean {
  const styleAttr = element.getAttribute("style") ?? "";
  if (edge === "before" ? STYLE_BREAK_BEFORE.test(styleAttr) : STYLE_BREAK_AFTER.test(styleAttr)) {
    return true;
  }

  const style = element.style;
  const pageBreak = style.getPropertyValue(`page-break-${edge}`).toLowerCase();
  const fragmentBreak = style.getPropertyValue(`break-${edge}`).toLowerCase();
  return PAGE_BREAK_VALUES.has(pageBreak) || PAGE_BREAK_VALUES.has(fragmentBreak);
}

function hasMsoPageBreak(element: HTMLElement): boolean {
  const styleAttr = element.getAttribute("style") ?? "";
  if (STYLE_MSO_BREAK.test(styleAttr)) {
    return true;
  }

  return (
    element.style.getPropertyValue("mso-break-type").toLowerCase() === "section-break" ||
    element.style.getPropertyValue("mso-break-type").toLowerCase() === "page-break"
  );
}

function wordSectionNumber(element: HTMLElement): number | null {
  for (const className of Array.from(element.classList)) {
    const match = WORD_SECTION_CLASS.exec(className);
    if (match) {
      return Number(match[1]);
    }
  }

  return null;
}

function shouldReplaceElementWithMarker(element: HTMLElement, isBreak: boolean): boolean {
  if (!isBreak) {
    return false;
  }

  const tag = element.tagName;
  if (tag === "BR" || tag === "HR") {
    return true;
  }

  return tag === "P" && !elementHasVisibleText(element);
}

function elementHasVisibleText(element: HTMLElement): boolean {
  const text = element.textContent?.replace(/\u00a0/g, " ").trim() ?? "";
  if (text.length > 0) {
    return true;
  }

  return element.querySelector("img, table, ul, ol") !== null;
}

function insertMarkerBefore(element: HTMLElement, doc: Document): void {
  if (isPageBreakMarker(previousElementSibling(element)) || !hasPrecedingContent(element)) {
    return;
  }

  element.before(createPageBreakMarker(doc));
}

function insertMarkerAfter(element: HTMLElement, doc: Document): void {
  if (isPageBreakMarker(nextElementSibling(element)) || !hasFollowingContent(element)) {
    return;
  }

  element.after(createPageBreakMarker(doc));
}

function replaceWithMarker(element: HTMLElement, doc: Document): void {
  if (isPageBreakMarker(previousElementSibling(element))) {
    element.remove();
    return;
  }

  if (!hasPrecedingContent(element)) {
    element.remove();
    return;
  }

  element.replaceWith(createPageBreakMarker(doc));
}

function previousElementSibling(node: Node): HTMLElement | null {
  let current = node.previousSibling;
  while (current) {
    if (current instanceof HTMLElement) {
      return current;
    }

    current = current.previousSibling;
  }

  return null;
}

function nextElementSibling(node: Node): HTMLElement | null {
  let current = node.nextSibling;
  while (current) {
    if (current instanceof HTMLElement) {
      return current;
    }

    current = current.nextSibling;
  }

  return null;
}

function hasPrecedingContent(node: Node): boolean {
  return hasContentToward(node, "previousSibling");
}

function hasFollowingContent(node: Node): boolean {
  return hasContentToward(node, "nextSibling");
}

function hasContentToward(node: Node, direction: "previousSibling" | "nextSibling"): boolean {
  let current: Node | null = node;
  while (current) {
    let sibling = current[direction];
    while (sibling) {
      if (isMeaningfulContent(sibling)) {
        return true;
      }

      sibling = sibling[direction];
    }

    const parent: Node | null = current.parentNode;
    if (parent === null || parent.nodeType === Node.DOCUMENT_NODE || parent.nodeName === "BODY") {
      return false;
    }

    current = parent;
  }

  return false;
}

function isMeaningfulContent(node: Node): boolean {
  if (isPageBreakMarker(node)) {
    return false;
  }

  if (node instanceof HTMLElement) {
    return elementHasVisibleText(node) || node.querySelector("img, table, ul, ol, p") !== null;
  }

  return (
    node.nodeType === Node.TEXT_NODE &&
    (node.textContent?.replace(/\u00a0/g, " ").trim() ?? "").length > 0
  );
}

function stripCssPageBreak(element: HTMLElement, edge: "before" | "after"): void {
  element.style.removeProperty(`page-break-${edge}`);
  element.style.removeProperty(`break-${edge}`);
}

function liftPageBreakMarkers(root: ParentNode): void {
  for (const marker of Array.from(root.querySelectorAll("[data-page-break='true']"))) {
    if (!(marker instanceof HTMLElement) || !marker.isConnected) {
      continue;
    }

    let current: HTMLElement = marker;
    while (
      current.parentElement &&
      current.parentElement !== root &&
      current.parentElement.tagName !== "BODY" &&
      isEmptyWrapperAround(current.parentElement, current)
    ) {
      current.parentElement.replaceWith(current);
    }

    const parent = current.parentElement;
    if (parent && parent.tagName === "P" && !isEmptyWrapperAround(parent, current)) {
      parent.before(current);
    }
  }
}

function isEmptyWrapperAround(parent: HTMLElement, keep: HTMLElement): boolean {
  for (const child of Array.from(parent.childNodes)) {
    if (child === keep) {
      continue;
    }

    if (child instanceof HTMLElement) {
      if (child.contains(keep)) {
        continue;
      }

      if (elementHasVisibleText(child) || child.querySelector("img, table")) {
        return false;
      }

      continue;
    }

    if (
      child.nodeType === Node.TEXT_NODE &&
      (child.textContent?.replace(/\u00a0/g, " ").trim() ?? "").length > 0
    ) {
      return false;
    }
  }

  return true;
}

function unwrapWordSectionWrappers(root: ParentNode): void {
  for (const element of snapshotElements(root)) {
    if (!element.isConnected || wordSectionNumber(element) === null) {
      continue;
    }

    element.replaceWith(...Array.from(element.childNodes));
  }
}

function collapseAdjacentPageBreaks(root: ParentNode): void {
  const markers = Array.from(root.querySelectorAll("[data-page-break='true']"));
  for (const marker of markers) {
    if (!(marker instanceof HTMLElement) || !marker.isConnected) {
      continue;
    }

    const previous = previousElementSibling(marker);
    if (isPageBreakMarker(previous)) {
      marker.remove();
    }
  }
}

function dropEdgeOnlyPageBreaks(root: ParentNode): void {
  const children = Array.from(root.childNodes);
  for (const child of children) {
    if (isPageBreakMarker(child) && !hasPrecedingContent(child)) {
      child.remove();
    }
  }

  const trailing = Array.from(root.childNodes).toReversed();
  for (const child of trailing) {
    if (isPageBreakMarker(child) && !hasFollowingContent(child)) {
      child.remove();
    }
  }
}

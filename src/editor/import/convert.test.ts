/**
 * @vitest-environment jsdom
 */
import { expect, it } from "@effect/vitest";
import { $getRoot, $isElementNode, createEditor } from "lexical";
import { insertSanitizedHtml, prepareClipboardDom } from "#/editor/import/convert";
import { EDITOR_NODES, EDITOR_THEME } from "#/editor/nodes/registry";
import { $isPageBreakNode } from "#/editor/nodes/page-break-node";
import { $isFieldNode } from "#/editor/nodes/field-node";

const WORD_PAGES = `<html xmlns:w="urn:schemas-microsoft-com:office:word">
<head>
<style>
@page WordSection1 { size: 595.3pt 841.9pt; }
div.WordSection1 { page: WordSection1; }
@page WordSection2 { size: 595.3pt 841.9pt; }
div.WordSection2 { page: WordSection2; }
</style>
</head>
<body>
<div class=WordSection1>
<p class=MsoNormal>First page body</p>
</div>
<span style="font-size:12.0pt"><br clear=all style="mso-special-character:line-break;page-break-before:always"></span>
<div class=WordSection2>
<p class=MsoNormal>Second page body</p>
</div>
</body>
</html>`;

const LIBREOFFICE_PAGES = `<p>Cover page</p>
<p style="page-break-before: always">Inner page</p>`;

const GOOGLE_DOCS_PAGES = `<p>Sheet one</p>
<hr style="page-break-before:always;display:none">
<p>Sheet two</p>`;

it("turns Word page-break BR and extra WordSection into markers without dropping later pages", () => {
  const { dom } = prepareClipboardDom(WORD_PAGES);
  const html = dom.body.innerHTML;
  expect(html).toContain("First page body");
  expect(html).toContain("Second page body");
  expect(dom.body.querySelectorAll("[data-page-break='true']")).toHaveLength(1);
});

it("keeps LibreOffice page content and inserts a break before the next page", () => {
  const { dom } = prepareClipboardDom(LIBREOFFICE_PAGES);
  expect(dom.body.textContent).toContain("Cover page");
  expect(dom.body.textContent).toContain("Inner page");
  expect(dom.body.querySelectorAll("[data-page-break='true']")).toHaveLength(1);
  const inner = [...dom.body.querySelectorAll("p")].find((node) =>
    (node.textContent ?? "").includes("Inner page"),
  );
  expect(inner?.style.pageBreakBefore).toBe("");
});

it("converts Google Docs page-break HR tags", () => {
  const { dom } = prepareClipboardDom(GOOGLE_DOCS_PAGES);
  expect(dom.body.querySelector("hr")).toBeNull();
  expect(dom.body.querySelectorAll("[data-page-break='true']")).toHaveLength(1);
  expect(dom.body.textContent).toContain("Sheet one");
  expect(dom.body.textContent).toContain("Sheet two");
});

it("leaves single-page HTML and field markers alone", () => {
  const { dom } = prepareClipboardDom(
    `<p>Only page</p><p><span data-field="true" data-field-reference='{"_tag":"UnresolvedToken","text":"fir_no"}'>fir_no</span></p>`,
  );
  expect(dom.body.querySelector("[data-page-break]")).toBeNull();
  expect(dom.body.querySelector("[data-field='true']")).not.toBeNull();
  expect(dom.body.textContent).toContain("Only page");
});

it("does not duplicate Missal page-break HTML", () => {
  const { dom } = prepareClipboardDom(
    `<p>One</p><div class="missal-page-break" data-page-break="true"></div><p>Two</p>`,
  );
  expect(dom.body.querySelectorAll("[data-page-break='true']")).toHaveLength(1);
});

it("stores every Word page and the page break in the Lexical envelope", () => {
  const editor = createEditor({
    namespace: "missal-page-break-test",
    nodes: [...EDITOR_NODES],
    onError: (error) => {
      throw error;
    },
    theme: EDITOR_THEME,
  });
  const root = document.createElement("div");
  document.body.append(root);
  editor.setRootElement(root);

  insertSanitizedHtml(editor, WORD_PAGES);

  editor.getEditorState().read(() => {
    const children = $getRoot().getChildren();
    const text = $getRoot().getTextContent();
    expect(text).toContain("First page body");
    expect(text).toContain("Second page body");
    expect(children.some($isPageBreakNode)).toBe(true);
  });

  root.remove();
});

it("still imports field HTML on a single page", () => {
  const editor = createEditor({
    namespace: "missal-field-paste-test",
    nodes: [...EDITOR_NODES],
    onError: (error) => {
      throw error;
    },
    theme: EDITOR_THEME,
  });
  const root = document.createElement("div");
  document.body.append(root);
  editor.setRootElement(root);

  insertSanitizedHtml(
    editor,
    `<p>Start <span data-field="true" data-field-reference='{"_tag":"UnresolvedToken","text":"fir_no"}'>fir_no</span> end</p>`,
  );

  editor.getEditorState().read(() => {
    const text = $getRoot().getTextContent();
    expect(text).toContain("Start");
    expect(text).toContain("end");
    const first = $getRoot().getChildren()[0];
    expect($isElementNode(first) && first.getChildren().some($isFieldNode)).toBe(true);
  });

  root.remove();
});

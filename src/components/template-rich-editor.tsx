import { useMemo } from "react";
import { CKEditor } from "@ckeditor/ckeditor5-react";
import {
  Alignment,
  AutoImage,
  Base64UploadAdapter,
  BlockQuote,
  Bold,
  ClassicEditor,
  Essentials,
  FontBackgroundColor,
  FontColor,
  FontFamily,
  FontSize,
  GeneralHtmlSupport,
  Heading,
  Highlight,
  HorizontalLine,
  Image,
  ImageCaption,
  ImageInsert,
  ImageResize,
  ImageStyle,
  ImageTextAlternative,
  ImageToolbar,
  ImageUpload,
  Indent,
  IndentBlock,
  Italic,
  Link,
  List,
  ListProperties,
  PageBreak,
  Paragraph,
  PasteFromOffice,
  RemoveFormat,
  SelectAll,
  SourceEditing,
  Strikethrough,
  Subscript,
  Superscript,
  Table,
  TableCaption,
  TableCellProperties,
  TableColumnResize,
  TableProperties,
  TableToolbar,
  Underline,
  Undo,
  type EditorConfig,
} from "ckeditor5";
import "ckeditor5/ckeditor5.css";
import { cn } from "#/lib/utils";

type TemplateRichEditorProps = {
  "aria-label": string;
  className?: string;
  id?: string;
  onChange: (value: string) => void;
  placeholder?: string;
  value: string;
};

const blockedTags = new Set(["script", "iframe", "object", "embed", "meta", "link", "title"]);
const blockedAttributes = [/^on/i, /^formaction$/i, /^srcdoc$/i];
const atPlaceholderPattern = /@([^@\r\n<>]{1,120})@/g;

function sanitizeTemplateHtml(value: string) {
  if (typeof DOMParser === "undefined") {
    return value;
  }

  const document = new DOMParser().parseFromString(value, "text/html");

  for (const element of Array.from(document.body.querySelectorAll("*"))) {
    if (blockedTags.has(element.tagName.toLowerCase())) {
      element.remove();
      continue;
    }

    for (const attribute of Array.from(element.attributes)) {
      const attributeName = attribute.name.toLowerCase();
      const isBlockedAttribute = blockedAttributes.some((pattern) => pattern.test(attribute.name));
      const isUnsafeUrl =
        (attributeName === "href" || attributeName === "src") &&
        /^\s*(javascript|data:text\/html):/i.test(attribute.value);
      const isUnsafeStyle =
        attributeName === "style" &&
        /expression\s*\(|javascript:|data:text\/html/i.test(attribute.value);

      if (isBlockedAttribute || isUnsafeUrl || isUnsafeStyle) {
        element.removeAttribute(attribute.name);
      }
    }
  }

  return document.body.innerHTML.trim();
}

function unwrapTemplatePlaceholderSpans(document: Document) {
  for (const element of Array.from(document.body.querySelectorAll('[data-placeholder="true"]'))) {
    element.replaceWith(document.createTextNode(element.textContent ?? ""));
  }
}

function markTemplatePlaceholdersHtml(value: string) {
  if (typeof DOMParser === "undefined") {
    return value;
  }

  const document = new DOMParser().parseFromString(value, "text/html");
  unwrapTemplatePlaceholderSpans(document);

  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const textNodes: Text[] = [];

  while (walker.nextNode()) {
    textNodes.push(walker.currentNode as Text);
  }

  for (const textNode of textNodes) {
    const text = textNode.nodeValue ?? "";

    if (!atPlaceholderPattern.test(text)) {
      atPlaceholderPattern.lastIndex = 0;
      continue;
    }

    atPlaceholderPattern.lastIndex = 0;
    const fragment = document.createDocumentFragment();
    let lastIndex = 0;

    for (const match of text.matchAll(atPlaceholderPattern)) {
      const index = match.index ?? 0;

      if (index > lastIndex) {
        fragment.append(document.createTextNode(text.slice(lastIndex, index)));
      }

      const placeholder = document.createElement("span");
      placeholder.dataset.placeholder = "true";
      placeholder.textContent = match[0];
      fragment.append(placeholder);
      lastIndex = index + match[0].length;
    }

    if (lastIndex < text.length) {
      fragment.append(document.createTextNode(text.slice(lastIndex)));
    }

    textNode.replaceWith(fragment);
  }

  return document.body.innerHTML.trim();
}

function normalizeTemplateHtml(value: string) {
  return markTemplatePlaceholdersHtml(sanitizeTemplateHtml(value));
}

const editorPlugins = [
  Alignment,
  AutoImage,
  Base64UploadAdapter,
  BlockQuote,
  Bold,
  Essentials,
  FontBackgroundColor,
  FontColor,
  FontFamily,
  FontSize,
  GeneralHtmlSupport,
  Heading,
  Highlight,
  HorizontalLine,
  Image,
  ImageCaption,
  ImageInsert,
  ImageResize,
  ImageStyle,
  ImageTextAlternative,
  ImageToolbar,
  ImageUpload,
  Indent,
  IndentBlock,
  Italic,
  Link,
  List,
  ListProperties,
  PageBreak,
  Paragraph,
  PasteFromOffice,
  RemoveFormat,
  SelectAll,
  SourceEditing,
  Strikethrough,
  Subscript,
  Superscript,
  Table,
  TableCaption,
  TableCellProperties,
  TableColumnResize,
  TableProperties,
  TableToolbar,
  Underline,
  Undo,
];

export function TemplateRichEditor({
  "aria-label": ariaLabel,
  className,
  id,
  onChange,
  placeholder,
  value,
}: TemplateRichEditorProps) {
  const editorId = id ?? "template-rich-editor";
  const editorValue = useMemo(() => normalizeTemplateHtml(value), [value]);
  const editorConfig = useMemo<EditorConfig>(
    () => ({
      alignment: {
        options: ["right", "center", "left", "justify"],
      },
      fontFamily: {
        supportAllValues: true,
      },
      fontSize: {
        options: [10, 12, 14, "default", 18, 20, 24, 28, 32],
        supportAllValues: true,
      },
      heading: {
        options: [
          { model: "paragraph", title: "Paragraph", class: "ck-heading_paragraph" },
          {
            model: "heading1",
            view: "h1",
            title: "Heading 1",
            class: "ck-heading_heading1",
          },
          {
            model: "heading2",
            view: "h2",
            title: "Heading 2",
            class: "ck-heading_heading2",
          },
          {
            model: "heading3",
            view: "h3",
            title: "Heading 3",
            class: "ck-heading_heading3",
          },
        ],
      },
      htmlSupport: {
        allow: [
          {
            name: /.*/,
            attributes: true,
            classes: true,
            styles: true,
          },
        ],
      },
      image: {
        toolbar: [
          "imageTextAlternative",
          "toggleImageCaption",
          "|",
          "imageStyle:inline",
          "imageStyle:block",
          "imageStyle:side",
          "|",
          "resizeImage",
        ],
      },
      language: {
        content: "ur",
      },
      licenseKey: "GPL",
      link: {
        addTargetToExternalLinks: true,
        defaultProtocol: "https://",
      },
      list: {
        properties: {
          reversed: true,
          startIndex: true,
          styles: true,
        },
      },
      plugins: editorPlugins,
      root: {
        placeholder,
      },
      table: {
        contentToolbar: [
          "tableColumn",
          "tableRow",
          "mergeTableCells",
          "toggleTableCaption",
          "tableProperties",
          "tableCellProperties",
        ],
      },
      toolbar: {
        items: [],
      },
    }),
    [placeholder],
  );

  return (
    <div
      className={cn("template-rich-editor h-full min-h-[24rem] min-w-0 max-w-full", className)}
      dir="rtl"
      lang="ur"
    >
      <CKEditor
        config={editorConfig}
        data={editorValue}
        editor={ClassicEditor}
        id={editorId}
        onChange={(_, editor) => {
          onChange(normalizeTemplateHtml(editor.getData()));
        }}
        onReady={(editor) => {
          const editableElement = editor.ui.view.editable.element;

          editableElement?.setAttribute("aria-label", ariaLabel);
          editableElement?.setAttribute("dir", "rtl");
          editableElement?.setAttribute("id", editorId);
          editableElement?.setAttribute("lang", "ur");
          editableElement?.setAttribute("spellcheck", "false");
        }}
      />
    </div>
  );
}

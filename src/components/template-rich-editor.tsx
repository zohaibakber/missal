import { useEffect, useRef } from "react";
import { cn } from "#/lib/utils";
import { escapeHtml } from "#/lib/templates";

type TemplateRichEditorProps = {
  "aria-label": string;
  className?: string;
  id?: string;
  onChange: (value: string) => void;
  placeholder?: string;
  value: string;
};

const blockedTags = new Set([
  "script",
  "iframe",
  "object",
  "embed",
  "meta",
  "style",
  "link",
  "title",
  "xml",
  "colgroup",
  "col",
]);
const blockedAttributes = [/^on/i, /^formaction$/i, /^srcdoc$/i];
const layoutProperties = new Set([
  "background",
  "background-color",
  "clear",
  "clip",
  "color",
  "display",
  "float",
  "font",
  "font-family",
  "font-weight",
  "height",
  "inset",
  "left",
  "margin",
  "margin-left",
  "margin-right",
  "max-height",
  "max-width",
  "min-height",
  "min-width",
  "overflow",
  "overflow-x",
  "padding-left",
  "padding-right",
  "position",
  "right",
  "table-layout",
  "transform",
  "white-space",
  "width",
  "word-break",
  "word-wrap",
]);

const fixedSizeAttributes = new Set([
  "cellpadding",
  "cellspacing",
  "height",
  "minwidth",
  "nowrap",
  "width",
]);

function sanitizeTemplateHtml(value: string) {
  const document = new DOMParser().parseFromString(value, "text/html");

  for (const element of Array.from(document.body.querySelectorAll("*"))) {
    if (blockedTags.has(element.tagName.toLowerCase())) {
      element.remove();
      continue;
    }

    for (const attribute of Array.from(element.attributes)) {
      const shouldRemove = blockedAttributes.some((pattern) => pattern.test(attribute.name));
      const isUnsafeUrl =
        ["href", "src"].includes(attribute.name.toLowerCase()) &&
        /^\s*javascript:/i.test(attribute.value);
      const isFixedSizeAttribute = fixedSizeAttributes.has(attribute.name.toLowerCase());

      if (shouldRemove || isUnsafeUrl || isFixedSizeAttribute) {
        element.removeAttribute(attribute.name);
      }
    }

    if (element instanceof HTMLElement) {
      for (const property of layoutProperties) {
        element.style.removeProperty(property);
      }

      if (element instanceof HTMLTableElement) {
        element.style.width = "100%";
        element.style.maxWidth = "100%";
        element.style.tableLayout = "fixed";
      }

      if (element instanceof HTMLTableCellElement) {
        element.style.minWidth = "0";
        element.style.maxWidth = "100%";
        element.style.overflowWrap = "anywhere";
        element.style.wordBreak = "break-word";
        element.style.whiteSpace = "normal";
      }
    }
  }

  return document.body.innerHTML;
}

function plainTextToHtml(value: string) {
  return escapeHtml(value).replace(/\n/g, "<br>");
}

function insertHtml(html: string) {
  const selection = window.getSelection();

  if (!selection?.rangeCount) {
    return;
  }

  selection.deleteFromDocument();
  const range = selection.getRangeAt(0);
  const fragment = range.createContextualFragment(html);
  const lastChild = fragment.lastChild;
  range.insertNode(fragment);

  if (lastChild) {
    range.setStartAfter(lastChild);
    range.collapse(true);
    selection.removeAllRanges();
    selection.addRange(range);
  }
}

function normalizeEditorContent(editor: HTMLDivElement) {
  const nextHtml = sanitizeTemplateHtml(editor.innerHTML);

  if (nextHtml !== editor.innerHTML) {
    editor.innerHTML = nextHtml;
  }

  return nextHtml;
}

export function TemplateRichEditor({
  "aria-label": ariaLabel,
  className,
  id,
  onChange,
  placeholder,
  value,
}: TemplateRichEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const isEmpty = !value.replace(/<[^>]*>/g, "").trim();

  useEffect(() => {
    const editor = editorRef.current;
    const nextValue = sanitizeTemplateHtml(value);

    if (nextValue !== value && document.activeElement !== editor) {
      onChange(nextValue);
      return;
    }

    if (!editor || document.activeElement === editor || editor.innerHTML === nextValue) {
      return;
    }

    editor.innerHTML = nextValue;
  }, [onChange, value]);

  return (
    <div className="relative min-w-0 max-w-full [contain:inline-size]">
      {isEmpty && placeholder ? (
        <div className="pointer-events-none absolute end-3 top-2 text-sm text-muted-foreground">
          {placeholder}
        </div>
      ) : null}
      <div
        ref={editorRef}
        aria-label={ariaLabel}
        className={cn(
          "min-h-[24rem] w-full min-w-0 max-w-full rounded-lg border border-input bg-background px-3 py-2 font-['Noto_Sans_Arabic_Variable',sans-serif] text-sm leading-7 font-normal text-foreground caret-foreground outline-none transition-colors [contain:inline-size] [overflow-wrap:anywhere] focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
          "prose prose-sm dark:prose-invert max-w-none [&_*]:!max-w-full [&_*]:!min-w-0 [&_*]:!whitespace-normal [&_*]:!font-['Noto_Sans_Arabic_Variable',sans-serif] [&_*]:!font-normal [&_*]:box-border [&_[data-placeholder=true]]:!font-medium [&_[data-placeholder=true]]:!text-blue-600 dark:[&_[data-placeholder=true]]:!text-blue-400 [&_b]:!font-semibold [&_img]:!h-auto [&_img]:!max-w-full [&_pre]:!whitespace-pre-wrap [&_strong]:!font-semibold [&_table]:!w-full [&_table]:!max-w-full [&_table]:table-fixed [&_table]:border-collapse [&_td]:min-w-0 [&_td]:border [&_td]:border-border [&_td]:p-1 [&_td]:break-words [&_th]:min-w-0 [&_th]:border [&_th]:border-border [&_th]:p-1 [&_th]:break-words",
          className,
        )}
        contentEditable
        dir="rtl"
        id={id}
        lang="ur"
        onBlur={(event) => onChange(sanitizeTemplateHtml(event.currentTarget.innerHTML))}
        onInput={(event) => onChange(event.currentTarget.innerHTML)}
        onPaste={(event) => {
          event.preventDefault();
          const html = event.clipboardData.getData("text/html");
          const text = event.clipboardData.getData("text/plain");
          const nextHtml = sanitizeTemplateHtml(html || plainTextToHtml(text));

          insertHtml(nextHtml);
          if (editorRef.current) {
            onChange(normalizeEditorContent(editorRef.current));
          }
        }}
        role="textbox"
        suppressContentEditableWarning
      />
    </div>
  );
}

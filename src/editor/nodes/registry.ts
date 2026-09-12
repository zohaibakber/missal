import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import { ListItemNode, ListNode } from "@lexical/list";
import { TableCellNode, TableNode, TableRowNode } from "@lexical/table";
import { LinkNode } from "@lexical/link";
import { HorizontalRuleNode } from "@lexical/react/LexicalHorizontalRuleNode";
import { LineBreakNode, ParagraphNode, TextNode, type Klass, type LexicalNode } from "lexical";
import { FieldNode } from "#/editor/nodes/field-node";
import { ImageNode } from "#/editor/nodes/image-node";
import { PageBreakNode } from "#/editor/nodes/page-break-node";

export const EDITOR_NODES: ReadonlyArray<Klass<LexicalNode>> = [
  ParagraphNode,
  TextNode,
  LineBreakNode,
  HeadingNode,
  QuoteNode,
  ListNode,
  ListItemNode,
  TableNode,
  TableRowNode,
  TableCellNode,
  LinkNode,
  HorizontalRuleNode,
  ImageNode,
  PageBreakNode,
  FieldNode,
];

export const EDITOR_THEME = {
  ltr: "missal-ltr",
  rtl: "missal-rtl",
  paragraph: "missal-paragraph",
  heading: {
    h1: "missal-h1",
    h2: "missal-h2",
    h3: "missal-h3",
    h4: "missal-h4",
    h5: "missal-h5",
    h6: "missal-h6",
  },
  list: {
    nested: { listitem: "missal-nested-listitem" },
    ol: "missal-ol",
    ul: "missal-ul",
    listitem: "missal-listitem",
  },
  quote: "missal-quote",
  link: "missal-link",
  text: {
    bold: "missal-text-bold",
    italic: "missal-text-italic",
    underline: "missal-text-underline",
    strikethrough: "missal-text-strike",
    subscript: "missal-text-sub",
    superscript: "missal-text-sup",
  },
  table: "missal-table",
  tableCell: "missal-table-cell",
  tableRow: "missal-table-row",
  hr: "missal-hr",
  image: "missal-image",
};

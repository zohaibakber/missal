import { $applyNodeReplacement, DecoratorNode } from "lexical";
import type {
  DOMConversionMap,
  DOMConversionOutput,
  DOMExportOutput,
  EditorConfig,
  LexicalNode,
} from "lexical";
import { htmlElementIsPageBreakImport } from "#/editor/import/page-breaks";

function convertPageBreakElement(element: HTMLElement): DOMConversionOutput | null {
  if (!htmlElementIsPageBreakImport(element)) {
    return null;
  }

  return { node: $createPageBreakNode() };
}

function pageBreakImport(node: HTMLElement) {
  if (!htmlElementIsPageBreakImport(node)) {
    return null;
  }

  return {
    conversion: convertPageBreakElement,
    priority: 3 as const,
  };
}

const pageBreakImportDOM: DOMConversionMap = {
  br: pageBreakImport,
  div: pageBreakImport,
  hr: pageBreakImport,
};

export class PageBreakNode extends DecoratorNode<null> {
  $config() {
    return this.config("pagebreak", {
      extends: DecoratorNode,
      importDOM: pageBreakImportDOM,
    });
  }

  isInline() {
    return false;
  }

  createDOM(_config: EditorConfig): HTMLElement {
    const divider = document.createElement("div");
    divider.className = "missal-page-break";
    divider.dataset.pageBreak = "true";
    return divider;
  }

  exportDOM(): DOMExportOutput {
    return { element: this.createDOM({} as EditorConfig) };
  }

  updateDOM(): false {
    return false;
  }

  decorate(): null {
    return null;
  }
}

export function $createPageBreakNode() {
  return $applyNodeReplacement(new PageBreakNode());
}

export function $isPageBreakNode(node: LexicalNode | null | undefined): node is PageBreakNode {
  return node instanceof PageBreakNode;
}

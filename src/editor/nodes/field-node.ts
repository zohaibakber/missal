import { Option, Schema } from "effect";
import {
  $applyNodeReplacement,
  $getState,
  $setState,
  createState,
  DecoratorNode,
  type DOMConversionMap,
  type DOMConversionOutput,
  type DOMExportOutput,
  type EditorConfig,
  type LexicalEditor,
  type LexicalNode,
} from "lexical";
import { FieldReference, UnresolvedTokenReference } from "#/lib/field";
import { paintFieldHost } from "#/editor/field-host";
import { getEditorPresentation } from "#/editor/presentation-context";

const parseFieldReference = (value: unknown): FieldReference => {
  const decoded = Schema.decodeUnknownOption(FieldReference)(value);
  return Option.getOrElse(decoded, () => UnresolvedTokenReference.make({ text: "" }));
};

const fieldReferenceState = createState("reference", {
  parse: parseFieldReference,
});

function convertFieldElement(element: HTMLElement): DOMConversionOutput | null {
  const raw = element.dataset.fieldReference;
  if (!raw) {
    return null;
  }

  try {
    return { node: $createFieldNode(parseFieldReference(JSON.parse(raw) as unknown)) };
  } catch {
    return null;
  }
}

const fieldImportDOM: DOMConversionMap = {
  span: (node) => {
    if (!(node instanceof HTMLElement) || node.dataset.field !== "true") {
      return null;
    }

    return {
      conversion: convertFieldElement,
      priority: 3,
    };
  },
};

export class FieldNode extends DecoratorNode<null> {
  $config() {
    return this.config("field", {
      extends: DecoratorNode,
      importDOM: fieldImportDOM,
      stateConfigs: [{ flat: true, stateConfig: fieldReferenceState }],
    });
  }

  isInline() {
    return true;
  }

  isKeyboardSelectable() {
    return true;
  }

  createDOM(_config: EditorConfig): HTMLElement {
    const host = document.createElement("span");
    host.className = "missal-field";
    host.dataset.field = "true";
    host.contentEditable = "false";
    host.setAttribute("dir", "auto");
    return host;
  }

  exportDOM(editor: LexicalEditor): DOMExportOutput {
    const host = this.createDOM({} as EditorConfig);
    const reference = this.getReference();
    host.dataset.fieldReference = JSON.stringify(reference);
    paintFieldHost(host, reference, getEditorPresentation(editor));
    return { element: host };
  }

  updateDOM(): false {
    return false;
  }

  decorate(): null {
    return null;
  }

  getReference() {
    return $getState(this, fieldReferenceState);
  }

  setReference(reference: FieldReference) {
    return $setState(this, fieldReferenceState, reference);
  }
}

export function $createFieldNode(reference: FieldReference): FieldNode {
  return $applyNodeReplacement($setState(new FieldNode(), fieldReferenceState, reference));
}

export function $isFieldNode(node: LexicalNode | null | undefined): node is FieldNode {
  return node instanceof FieldNode;
}

export function $getFieldReference(node: FieldNode) {
  return $getState(node, fieldReferenceState);
}

import type { LexicalEditor, NodeKey } from "lexical";
import { $getNodeByKey } from "lexical";
import type { FieldPresentationContext } from "#/lib/field";
import { paintFieldHost } from "#/editor/field-host";
import { FieldNode, $getFieldReference } from "#/editor/nodes/field-node";
import { setEditorPresentation } from "#/editor/presentation-context";

export type { FieldPresentationContext };

export class FieldPresentationController {
  private readonly hosts = new Map<NodeKey, HTMLElement>();
  private context: FieldPresentationContext;

  constructor(
    readonly editor: LexicalEditor,
    context: FieldPresentationContext,
  ) {
    this.context = context;
    setEditorPresentation(editor, context);
  }

  attach() {
    return this.editor.registerMutationListener(FieldNode, (mutations) => {
      for (const [key, mutation] of mutations) {
        if (mutation === "destroyed") {
          this.hosts.delete(key);
          continue;
        }

        const host = this.editor.getElementByKey(key);
        if (host) {
          this.hosts.set(key, host);
          this.renderHost(key, host);
        }
      }
    });
  }

  setContext(context: FieldPresentationContext) {
    this.context = context;
    setEditorPresentation(this.editor, context);
    for (const [key, host] of this.hosts) {
      this.renderHost(key, host);
    }
  }

  private renderHost(key: NodeKey, host: HTMLElement) {
    this.editor.getEditorState().read(() => {
      const node = $getNodeByKey(key);
      if (!(node instanceof FieldNode)) {
        return;
      }

      paintFieldHost(host, $getFieldReference(node), this.context);
    });
  }
}

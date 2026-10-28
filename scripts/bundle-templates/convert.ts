import { $generateNodesFromDOM } from "@lexical/html";
import { $getRoot, $insertNodes, createEditor } from "lexical";
import { captureEditorEnvelope } from "#/editor/envelope";
import { EDITOR_HTML_CONFIG } from "#/editor/html-config";
import { prepareClipboardDom } from "#/editor/import/convert";
import { importDocx } from "#/editor/import/docx";
import { EDITOR_NODES } from "#/editor/nodes/registry";
import { registerCompletedTokenConversion } from "#/editor/recognition";
import { CustomSource, SharedSettingSource } from "#/lib/field";
import { PlaceholderId } from "#/lib/ids";
import { indexFields, Placeholder } from "#/lib/placeholder";
import { DEFAULT_FIELD_MARKERS } from "#/lib/settings";
import { type TemplateBundleFields, unbindCatalogFields } from "#/lib/template-bundle";

async function convertTemplate(bytes: Uint8Array<ArrayBuffer>, fields: TemplateBundleFields) {
  const placeholders = [
    ...fields.global.map(
      (label, index) =>
        new Placeholder({
          id: PlaceholderId.make(index + 1),
          label,
          source: SharedSettingSource.make({ setting: `bundle_${index}` }),
        }),
    ),
    ...fields.custom.map(
      (label, index) =>
        new Placeholder({
          id: PlaceholderId.make(fields.global.length + index + 1),
          label,
          source: CustomSource.make({}),
        }),
    ),
  ];
  const index = indexFields(placeholders);
  const imported = await importDocx(new File([bytes], "template.docx"), {
    markers: DEFAULT_FIELD_MARKERS,
    index,
  });
  const editor = createEditor({
    namespace: "missal-template-bundle",
    html: EDITOR_HTML_CONFIG,
    nodes: [...EDITOR_NODES],
    onError: (error) => {
      throw error;
    },
  });
  const unregister = registerCompletedTokenConversion(
    editor,
    () => index,
    () => DEFAULT_FIELD_MARKERS,
  );
  const { dom } = prepareClipboardDom(imported.html);
  editor.update(
    () => {
      const root = $getRoot();
      root.clear();
      root.selectStart();
      $insertNodes($generateNodesFromDOM(editor, dom));
    },
    { discrete: true },
  );
  unregister();
  const { format, version, pageLayout, state } = captureEditorEnvelope(editor, imported.pageLayout);
  const labels = new Map(placeholders.map(({ id, label }) => [id as number, label]));
  return { format, version, pageLayout, state: unbindCatalogFields(state, (id) => labels.get(id)) };
}

Object.assign(window, { convertTemplate });

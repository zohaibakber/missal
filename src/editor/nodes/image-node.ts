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
} from "lexical";

const imageSrcState = createState("src", {
  parse: (value: unknown) => (typeof value === "string" ? value : ""),
});
const imageWidthState = createState("width", {
  parse: (value: unknown) => (typeof value === "number" ? value : null),
});
const imageHeightState = createState("height", {
  parse: (value: unknown) => (typeof value === "number" ? value : null),
});

function convertImageElement(element: HTMLElement): DOMConversionOutput | null {
  if (!(element instanceof HTMLImageElement)) {
    return null;
  }

  const src = element.getAttribute("src") ?? "";
  if (!src.startsWith("data:image/")) {
    return null;
  }

  const width = element.width > 0 ? element.width : null;
  const height = element.height > 0 ? element.height : null;
  return { node: $createImageNode(src, width, height) };
}

const imageImportDOM: DOMConversionMap = {
  img: (node) => {
    if (!(node instanceof HTMLImageElement) || !node.src.startsWith("data:image/")) {
      return null;
    }

    return {
      conversion: convertImageElement,
      priority: 2,
    };
  },
};

export class ImageNode extends DecoratorNode<null> {
  $config() {
    return this.config("image", {
      extends: DecoratorNode,
      importDOM: imageImportDOM,
      stateConfigs: [
        { flat: true, stateConfig: imageSrcState },
        { flat: true, stateConfig: imageWidthState },
        { flat: true, stateConfig: imageHeightState },
      ],
    });
  }

  isInline() {
    return false;
  }

  createDOM(_config: EditorConfig): HTMLElement {
    const figure = document.createElement("figure");
    figure.className = "missal-image";
    const image = document.createElement("img");
    image.src = $getState(this, imageSrcState);
    const width = $getState(this, imageWidthState);
    const height = $getState(this, imageHeightState);
    if (typeof width === "number") {
      image.width = width;
    }
    if (typeof height === "number") {
      image.height = height;
    }
    image.alt = "";
    figure.append(image);
    return figure;
  }

  updateDOM(): false {
    return false;
  }

  exportDOM(): DOMExportOutput {
    return { element: this.createDOM({} as EditorConfig) };
  }

  decorate(): null {
    return null;
  }
}

function $createImageNode(src: string, width: number | null = null, height: number | null = null) {
  return $applyNodeReplacement(
    $setState(
      $setState($setState(new ImageNode(), imageSrcState, src), imageWidthState, width),
      imageHeightState,
      height,
    ),
  );
}

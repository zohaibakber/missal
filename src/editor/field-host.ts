import { fieldDisplayText, type FieldPresentationContext, type FieldReference } from "#/lib/field";

export function paintFieldHost(
  host: HTMLElement,
  reference: FieldReference,
  context: FieldPresentationContext | undefined,
) {
  host.replaceChildren();
  if (!context) {
    return;
  }

  const display = fieldDisplayText(reference, context);
  host.classList.toggle("missal-field-unresolved", display.unresolved);
  host.title = display.unresolved ? "Unresolved field" : "";

  const text = document.createElement("span");
  text.className = "missal-field-value";
  text.style.unicodeBidi = "isolate";
  text.style.whiteSpace = "pre-wrap";
  text.textContent = display.text;
  host.append(text);
}

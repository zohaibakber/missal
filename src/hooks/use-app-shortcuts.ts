import { useNavigate } from "@tanstack/react-router";
import { useSequenceShortcut, useShortcut } from "#/hooks/use-shortcut";

/** App-wide shortcuts that work on every page. Page-specific shortcuts live with their page. */
export function useAppShortcuts() {
  const navigate = useNavigate();

  useSequenceShortcut("goHome", () => void navigate({ to: "/" }));
  useSequenceShortcut("goTemplates", () => void navigate({ to: "/templates" }));
  useSequenceShortcut("goPlaceholders", () => void navigate({ to: "/placeholders" }));
  useSequenceShortcut("goSettings", () => void navigate({ to: "/settings" }));
  useShortcut("newFir", () => void navigate({ to: "/new" }));
  useShortcut("newTemplate", () => void navigate({ to: "/templates/new" }));

  // Escape hands focus back to the page so single-key shortcuts work again.
  useShortcut(
    "blurEditor",
    () => {
      const active = document.activeElement;
      if (active instanceof HTMLElement && active !== document.body) active.blur();
    },
    { ignoreInputs: false },
  );
}

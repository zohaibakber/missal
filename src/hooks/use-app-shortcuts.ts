import { useNavigate } from "@tanstack/react-router";
import { useSequenceShortcut, useShortcut } from "#/hooks/use-shortcut";

export function useAppShortcuts() {
  const navigate = useNavigate();

  useSequenceShortcut("goHome", () => void navigate({ to: "/" }));
  useSequenceShortcut("goTemplates", () => void navigate({ to: "/templates" }));
  useSequenceShortcut("goSettings", () => void navigate({ to: "/settings" }));
  useShortcut("newFir", () => void navigate({ to: "/new" }));
  useShortcut("newTemplate", () => void navigate({ to: "/templates/new" }));

  useShortcut(
    "blurEditor",
    () => {
      const active = document.activeElement;
      if (active instanceof HTMLElement && active !== document.body) active.blur();
    },
    { ignoreInputs: false },
  );
}

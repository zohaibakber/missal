import { useHotkey, type HotkeyCallback, type UseHotkeyOptions } from "@tanstack/react-hotkeys";
import { shortcuts, type ShortcutId } from "#/lib/shortcuts";

const OVERLAY_SELECTOR = ["dialog", "alertdialog", "menu", "listbox"]
  .map((role) => `[role="${role}"][data-open]`)
  .join(",");

function isOverlayOpen() {
  return document.querySelector(OVERLAY_SELECTOR) !== null;
}

type UseShortcutOptions = Pick<UseHotkeyOptions, "enabled" | "ignoreInputs" | "target"> & {
  allowInOverlay?: boolean;
};

const CONFLICT_BEHAVIOR = "allow";

export function useShortcut(
  id: ShortcutId,
  callback: HotkeyCallback,
  { allowInOverlay = false, ...options }: UseShortcutOptions = {},
) {
  useHotkey(
    shortcuts[id].keys,
    (event, context) => {
      if (!allowInOverlay && isOverlayOpen()) return;
      callback(event, context);
    },
    { conflictBehavior: CONFLICT_BEHAVIOR, ...options },
  );
}

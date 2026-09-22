import {
  useHotkey,
  useHotkeySequence,
  type HotkeyCallback,
  type UseHotkeyOptions,
} from "@tanstack/react-hotkeys";
import { shortcuts } from "#/lib/shortcuts";

type Shortcuts = typeof shortcuts;
export type ChordShortcutId = {
  [Id in keyof Shortcuts]: Shortcuts[Id]["keys"] extends string ? Id : never;
}[keyof Shortcuts];
export type SequenceShortcutId = Exclude<keyof Shortcuts, ChordShortcutId>;

const OVERLAY_SELECTOR = ["dialog", "alertdialog", "menu", "listbox"]
  .map((role) => `[role="${role}"][data-open]`)
  .join(",");

/** True while a modal, menu or listbox is open, so page shortcuts don't fire behind it. */
export function isOverlayOpen() {
  return document.querySelector(OVERLAY_SELECTOR) !== null;
}

type UseShortcutOptions = Pick<UseHotkeyOptions, "enabled" | "ignoreInputs" | "target"> & {
  /** Fire even while a dialog or menu is open. */
  allowInOverlay?: boolean;
};

// Several handlers may share a key: pages briefly overlap during route transitions, and the
// Placeholders tabs stay mounted with only the active tab's Save enabled. `enabled` and mounting
// decide which one acts, so keep every registration instead of warning or replacing.
const CONFLICT_BEHAVIOR = "allow";

function guard(callback: HotkeyCallback, allowInOverlay: boolean): HotkeyCallback {
  return (event, context) => {
    if (!allowInOverlay && isOverlayOpen()) return;
    callback(event, context);
  };
}

/** Binds a chord from the shared registry, so its keys always match what the UI displays. */
export function useShortcut(
  id: ChordShortcutId,
  callback: HotkeyCallback,
  { allowInOverlay = false, ...options }: UseShortcutOptions = {},
) {
  useHotkey(shortcuts[id].keys, guard(callback, allowInOverlay), {
    conflictBehavior: CONFLICT_BEHAVIOR,
    ...options,
  });
}

/** Binds a Vim-style key sequence (e.g. `G` then `H`) from the shared registry. */
export function useSequenceShortcut(
  id: SequenceShortcutId,
  callback: HotkeyCallback,
  { allowInOverlay = false, ...options }: UseShortcutOptions = {},
) {
  useHotkeySequence([...shortcuts[id].keys], guard(callback, allowInOverlay), {
    conflictBehavior: CONFLICT_BEHAVIOR,
    ...options,
  });
}

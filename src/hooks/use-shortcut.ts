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

function isOverlayOpen() {
  return document.querySelector(OVERLAY_SELECTOR) !== null;
}

type UseShortcutOptions = Pick<UseHotkeyOptions, "enabled" | "ignoreInputs" | "target"> & {
  allowInOverlay?: boolean;
};

// Handlers can share a key (overlapping route transitions, tabs that stay mounted); `enabled` decides.
const CONFLICT_BEHAVIOR = "allow";

function guard(callback: HotkeyCallback, allowInOverlay: boolean): HotkeyCallback {
  return (event, context) => {
    if (!allowInOverlay && isOverlayOpen()) return;
    callback(event, context);
  };
}

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

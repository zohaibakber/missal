import { formatForDisplay } from "@tanstack/react-hotkeys";
import { Kbd, KbdGroup } from "#/components/ui/kbd";
import { shortcuts, type ShortcutId } from "#/lib/shortcuts";
import { cn } from "#/lib/utils";

/**
 * Platform-aware key hint (`Ctrl S` / `⌘ S`) for a shortcut from the shared registry.
 * Hidden from assistive tech so it doesn't pollute button names; use `shortcutLabel` for text.
 */
export function ShortcutKbd({ id, className }: { id: ShortcutId; className?: string }) {
  const { keys } = shortcuts[id];
  const chords = typeof keys === "string" ? [keys] : keys;

  return (
    <KbdGroup aria-hidden="true" dir="ltr" className={cn("shrink-0", className)}>
      {chords.map((chord, chordIndex) =>
        formatForDisplay(chord, { parts: true }).map((part, index) => (
          <Kbd key={`${chordIndex}-${index}`}>{part}</Kbd>
        )),
      )}
    </KbdGroup>
  );
}

/** Keys as plain text, e.g. "Ctrl+S" or "G then H". */
export function shortcutKeysText(id: ShortcutId) {
  const { keys } = shortcuts[id];
  const chords = typeof keys === "string" ? [keys] : keys;
  return chords.map((chord) => formatForDisplay(chord, { useSymbols: false })).join(" then ");
}

/** Tooltip text such as "Save (Ctrl+S)". */
export function shortcutLabel(id: ShortcutId, label: string = shortcuts[id].label) {
  return `${label} (${shortcutKeysText(id)})`;
}

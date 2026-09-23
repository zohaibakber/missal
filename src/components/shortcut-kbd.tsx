import { formatForDisplay } from "@tanstack/react-hotkeys";
import { Kbd, KbdGroup } from "#/components/ui/kbd";
import { shortcuts, type ShortcutId } from "#/lib/shortcuts";
import { cn } from "#/lib/utils";

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

export function shortcutKeysText(id: ShortcutId) {
  const { keys } = shortcuts[id];
  const chords = typeof keys === "string" ? [keys] : keys;
  return chords.map((chord) => formatForDisplay(chord, { useSymbols: false })).join(" then ");
}

export function shortcutLabel(id: ShortcutId, label: string = shortcuts[id].label) {
  return `${label} (${shortcutKeysText(id)})`;
}

import { createContext, use, useState, type ReactNode } from "react";
import { ShortcutKbd, shortcutKeysText } from "#/components/shortcut-kbd";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "#/components/ui/dialog";
import { useShortcut } from "#/hooks/use-shortcut";
import { SHORTCUT_GROUPS, shortcuts, type ShortcutId } from "#/lib/shortcuts";

type KeyboardShortcutsContextValue = {
  openShortcuts: () => void;
};

const KeyboardShortcutsContext = createContext<KeyboardShortcutsContextValue | null>(null);

export function useKeyboardShortcuts() {
  const context = use(KeyboardShortcutsContext);
  if (!context) {
    throw new Error("useKeyboardShortcuts must be used within KeyboardShortcutsProvider.");
  }
  return context;
}

const shortcutsByGroup = SHORTCUT_GROUPS.map((group) => ({
  group,
  ids: (Object.keys(shortcuts) as ShortcutId[]).filter((id) => shortcuts[id].group === group),
}));

export function KeyboardShortcutsProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);

  useShortcut("cheatsheet", () => setOpen((value) => !value), { allowInOverlay: open });

  return (
    <KeyboardShortcutsContext value={{ openShortcuts: () => setOpen(true) }}>
      {children}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-h-[calc(100dvh-4rem)] overflow-y-auto sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>Keyboard shortcuts</DialogTitle>
            <DialogDescription>
              Single-key shortcuts work when no text field or document is focused. Press{" "}
              <ShortcutKbd id="blurEditor" /> to leave one.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-x-8 gap-y-6 sm:grid-cols-2">
            {shortcutsByGroup.map(({ group, ids }) => (
              <section key={group} className="flex flex-col gap-2">
                <h3 className="text-xs font-medium text-muted-foreground">{group}</h3>
                <dl className="flex flex-col gap-1.5">
                  {ids.map((id) => (
                    <div key={id} className="flex items-center justify-between gap-4">
                      <dt>{shortcuts[id].label}</dt>
                      <dd>
                        <ShortcutKbd id={id} />
                        <span className="sr-only">{shortcutKeysText(id)}</span>
                      </dd>
                    </div>
                  ))}
                </dl>
              </section>
            ))}
          </div>
        </DialogContent>
      </Dialog>
    </KeyboardShortcutsContext>
  );
}

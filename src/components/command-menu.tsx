import { lazy, Suspense, useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Search01Icon } from "@hugeicons/core-free-icons";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Button } from "#/components/ui/button";
import { useShortcut } from "#/hooks/use-shortcut";

const loadDialog = () => import("#/components/command-menu-dialog");
const CommandMenuDialog = lazy(() =>
  loadDialog().then((module) => ({ default: module.CommandMenuDialog })),
);

export function CommandMenu() {
  const [open, setOpen] = useState(false);
  const [loaded, setLoaded] = useState(false);

  function toggle(next: (open: boolean) => boolean) {
    setLoaded(true);
    setOpen(next);
  }

  useShortcut("commandMenu", () => toggle((value) => !value), { allowInOverlay: open });

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        className="w-56 justify-start"
        onPointerEnter={() => void loadDialog()}
        onClick={() => toggle(() => true)}
      >
        <HugeiconsIcon icon={Search01Icon} data-icon="inline-start" />
        Search or jump to…
        <ShortcutKbd id="commandMenu" className="ms-auto" />
      </Button>

      {loaded ? (
        <Suspense fallback={null}>
          <CommandMenuDialog open={open} onOpenChange={setOpen} />
        </Suspense>
      ) : null}
    </>
  );
}

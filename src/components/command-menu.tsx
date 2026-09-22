import { Fragment, useState, type ComponentProps } from "react";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { useNavigate } from "@tanstack/react-router";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  ComputerIcon,
  File01Icon,
  Files,
  Home,
  KeyboardIcon,
  LegalDocument01Icon,
  Moon02Icon,
  Search01Icon,
  Settings01Icon,
  SidebarLeftIcon,
  Sun03Icon,
  TextFontIcon,
} from "@hugeicons/core-free-icons";
import { useKeyboardShortcuts } from "#/components/keyboard-shortcuts";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { useTheme } from "#/components/theme-provider";
import { Button } from "#/components/ui/button";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "#/components/ui/command";
import { useSidebar } from "#/components/ui/sidebar";
import { useShortcut } from "#/hooks/use-shortcut";
import type { ShortcutId } from "#/lib/shortcuts";
import { atoms } from "#/state/atoms";

type HugeIcon = ComponentProps<typeof HugeiconsIcon>["icon"];

type CommandLink = {
  icon: HugeIcon;
  shortcut: ShortcutId;
  title: string;
  to: "/" | "/templates" | "/placeholders" | "/settings" | "/new" | "/templates/new";
};

const navigationCommands: readonly CommandLink[] = [
  { icon: Home, shortcut: "goHome", title: "FIRs", to: "/" },
  { icon: Files, shortcut: "goTemplates", title: "Templates", to: "/templates" },
  { icon: TextFontIcon, shortcut: "goPlaceholders", title: "Placeholders", to: "/placeholders" },
  { icon: Settings01Icon, shortcut: "goSettings", title: "Settings", to: "/settings" },
];

const createCommands: readonly CommandLink[] = [
  { icon: Add01Icon, shortcut: "newFir", title: "New FIR", to: "/new" },
  { icon: Add01Icon, shortcut: "newTemplate", title: "New template", to: "/templates/new" },
];

const themeCommands = [
  { icon: Sun03Icon, title: "Light theme", value: "light" },
  { icon: Moon02Icon, title: "Dark theme", value: "dark" },
  { icon: ComputerIcon, title: "System theme", value: "system" },
] as const;

export function CommandMenu() {
  const navigate = useNavigate();
  const { openShortcuts } = useKeyboardShortcuts();
  const { setTheme, theme } = useTheme();
  const { toggleSidebar } = useSidebar();
  const firs = useAtomValue(atoms.firsAtom);
  const templates = useAtomValue(atoms.templatesAtom);
  const [open, setOpen] = useState(false);

  useShortcut("commandMenu", () => setOpen((value) => !value), { allowInOverlay: open });

  function run(action: () => void) {
    setOpen(false);
    action();
  }

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        className="w-56 justify-start"
        onClick={() => setOpen(true)}
      >
        <HugeiconsIcon icon={Search01Icon} data-icon="inline-start" />
        Search or jump to…
        <ShortcutKbd id="commandMenu" className="ms-auto" />
      </Button>

      <CommandDialog className="sm:max-w-xl" open={open} onOpenChange={setOpen}>
        <Command>
          <CommandInput placeholder="Search FIRs, templates and actions…" />
          <CommandList className="max-h-96">
            <CommandEmpty>No results found.</CommandEmpty>
            {[
              { heading: "Go to", items: navigationCommands },
              { heading: "Create", items: createCommands },
            ].map((section) => (
              <Fragment key={section.heading}>
                <CommandGroup heading={section.heading}>
                  {section.items.map((item) => (
                    <CommandItem
                      key={item.to}
                      onSelect={() => run(() => void navigate({ to: item.to }))}
                    >
                      <HugeiconsIcon icon={item.icon} />
                      <span>{item.title}</span>
                      <CommandShortcut>
                        <ShortcutKbd id={item.shortcut} />
                      </CommandShortcut>
                    </CommandItem>
                  ))}
                </CommandGroup>
                <CommandSeparator />
              </Fragment>
            ))}

            {AsyncResult.isSuccess(firs) && firs.value.length ? (
              <>
                <CommandGroup heading="FIRs">
                  {firs.value.map((fir) => (
                    <CommandItem
                      key={fir.id}
                      value={`fir ${fir.fir_no} ${fir.offence} ${fir.id}`}
                      onSelect={() =>
                        run(() => void navigate({ to: "/$firId", params: { firId: `${fir.id}` } }))
                      }
                    >
                      <HugeiconsIcon icon={File01Icon} />
                      <span className="font-mono">FIR {fir.fir_no}</span>
                      <span lang="ur" dir="rtl" className="ms-auto truncate text-muted-foreground">
                        {fir.offence}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
                <CommandSeparator />
              </>
            ) : null}

            {AsyncResult.isSuccess(templates) && templates.value.length ? (
              <>
                <CommandGroup heading="Templates">
                  {templates.value.map((template) => (
                    <CommandItem
                      key={template.id}
                      value={`template ${template.name} ${template.id}`}
                      onSelect={() =>
                        run(
                          () =>
                            void navigate({
                              to: "/templates/$templateId",
                              params: { templateId: `${template.id}` },
                            }),
                        )
                      }
                    >
                      <HugeiconsIcon icon={LegalDocument01Icon} />
                      <span lang="ur" dir="rtl" className="truncate">
                        {template.name}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
                <CommandSeparator />
              </>
            ) : null}

            <CommandGroup heading="Preferences">
              {themeCommands.map((item) => (
                <CommandItem
                  key={item.value}
                  data-checked={theme === item.value}
                  onSelect={() => run(() => setTheme(item.value))}
                >
                  <HugeiconsIcon icon={item.icon} />
                  <span>{item.title}</span>
                </CommandItem>
              ))}
              <CommandItem onSelect={() => run(toggleSidebar)}>
                <HugeiconsIcon icon={SidebarLeftIcon} />
                <span>Toggle sidebar</span>
                <CommandShortcut>
                  <ShortcutKbd id="toggleSidebar" />
                </CommandShortcut>
              </CommandItem>
              <CommandItem onSelect={() => run(openShortcuts)}>
                <HugeiconsIcon icon={KeyboardIcon} />
                <span>Keyboard shortcuts</span>
                <CommandShortcut>
                  <ShortcutKbd id="cheatsheet" />
                </CommandShortcut>
              </CommandItem>
            </CommandGroup>
          </CommandList>
        </Command>
      </CommandDialog>
    </>
  );
}

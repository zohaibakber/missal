import { Fragment, useDeferredValue, useState, type ComponentProps } from "react";
import { defaultFilter } from "cmdk";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { useNavigate } from "@tanstack/react-router";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  ComputerIcon,
  Folder01Icon,
  KeyboardIcon,
  LegalDocument01Icon,
  Moon02Icon,
  Settings01Icon,
  SidebarLeftIcon,
  Sun03Icon,
  TextFontIcon,
} from "@hugeicons/core-free-icons";
import { useKeyboardShortcuts } from "#/components/keyboard-shortcuts";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { useTheme } from "#/components/theme-provider";
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
import type { FirRecord } from "#/lib/fir";
import type { ShortcutId } from "#/lib/shortcuts";
import type { TemplateSummary } from "#/lib/templates";
import { atoms } from "#/state/atoms";

type HugeIcon = ComponentProps<typeof HugeiconsIcon>["icon"];

type CommandLink = {
  icon: HugeIcon;
  shortcut: ShortcutId;
  title: string;
  to: "/" | "/templates" | "/placeholders" | "/settings" | "/new" | "/templates/new";
};

const navigationCommands: readonly CommandLink[] = [
  { icon: Folder01Icon, shortcut: "goHome", title: "FIRs", to: "/" },
  { icon: LegalDocument01Icon, shortcut: "goTemplates", title: "Templates", to: "/templates" },
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

const MAX_RESULTS = 20;

const firValue = (fir: FirRecord) => `fir ${fir.fir_no} ${fir.offence} ${fir.id}`;
const templateValue = (template: TemplateSummary) => `template ${template.name} ${template.id}`;

function topMatches<T>(items: readonly T[], search: string, value: (item: T) => string) {
  if (!search) return items.slice(0, MAX_RESULTS);
  return items
    .map((item) => ({ item, score: defaultFilter(value(item), search) }))
    .filter((match) => match.score > 0)
    .toSorted((first, second) => second.score - first.score)
    .slice(0, MAX_RESULTS)
    .map((match) => match.item);
}

export function CommandMenuDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <CommandDialog className="sm:max-w-xl" open={open} onOpenChange={onOpenChange}>
      <CommandMenuContent close={() => onOpenChange(false)} />
    </CommandDialog>
  );
}

function CommandMenuContent({ close }: { close: () => void }) {
  const navigate = useNavigate();
  const { openShortcuts } = useKeyboardShortcuts();
  const { setTheme, theme } = useTheme();
  const { toggleSidebar } = useSidebar();
  const firs = useAtomValue(atoms.firsAtom);
  const templates = useAtomValue(atoms.templatesAtom);
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const firMatches = AsyncResult.isSuccess(firs)
    ? topMatches(firs.value, deferredSearch, firValue)
    : [];
  const templateMatches = AsyncResult.isSuccess(templates)
    ? topMatches(templates.value, deferredSearch, templateValue)
    : [];

  function run(action: () => void) {
    close();
    action();
  }

  return (
    <Command>
      <CommandInput
        placeholder="Search FIRs, templates and commands…"
        value={search}
        onValueChange={setSearch}
      />
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
                  <HugeiconsIcon icon={item.icon} strokeWidth={2} />
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

        {firMatches.length ? (
          <>
            <CommandGroup heading="FIRs">
              {firMatches.map((fir) => (
                <CommandItem
                  key={fir.id}
                  value={firValue(fir)}
                  onSelect={() =>
                    run(() => void navigate({ to: "/$firId", params: { firId: `${fir.id}` } }))
                  }
                >
                  <HugeiconsIcon icon={Folder01Icon} strokeWidth={2} />
                  <span>FIR {fir.fir_no}</span>
                  <span lang="ur" dir="rtl" className="ms-auto truncate text-muted-foreground">
                    {fir.offence}
                  </span>
                </CommandItem>
              ))}
            </CommandGroup>
            <CommandSeparator />
          </>
        ) : null}

        {templateMatches.length ? (
          <>
            <CommandGroup heading="Templates">
              {templateMatches.map((template) => (
                <CommandItem
                  key={template.id}
                  value={templateValue(template)}
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
                  <HugeiconsIcon icon={LegalDocument01Icon} strokeWidth={2} />
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
              <HugeiconsIcon icon={item.icon} strokeWidth={2} />
              <span>{item.title}</span>
            </CommandItem>
          ))}
          <CommandItem onSelect={() => run(toggleSidebar)}>
            <HugeiconsIcon icon={SidebarLeftIcon} strokeWidth={2} />
            <span>Toggle sidebar</span>
            <CommandShortcut>
              <ShortcutKbd id="toggleSidebar" />
            </CommandShortcut>
          </CommandItem>
          <CommandItem onSelect={() => run(openShortcuts)}>
            <HugeiconsIcon icon={KeyboardIcon} strokeWidth={2} />
            <span>Keyboard shortcuts</span>
            <CommandShortcut>
              <ShortcutKbd id="cheatsheet" />
            </CommandShortcut>
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </Command>
  );
}

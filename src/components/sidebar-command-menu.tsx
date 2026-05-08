import * as React from "react";
import { useNavigate } from "@tanstack/react-router";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  Database,
  Files,
  Home,
  Search01Icon,
  Settings05Icon,
} from "@hugeicons/core-free-icons";

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
import { InputGroup, InputGroupAddon, InputGroupInput } from "#/components/ui/input-group";
import { Kbd, KbdGroup } from "#/components/ui/kbd";
import { SidebarGroup, SidebarGroupContent } from "#/components/ui/sidebar";

const commandActions = [
  {
    group: "Navigation",
    items: [
      {
        icon: Home,
        shortcut: "H",
        title: "Home",
        url: "/",
      },
      {
        icon: Database,
        shortcut: "D",
        title: "Dataset",
        url: "/dataset",
      },
      {
        icon: Files,
        shortcut: "T",
        title: "Templates",
        url: "/templates",
      },
      {
        icon: Settings05Icon,
        shortcut: "S",
        title: "Settings",
        url: "/settings",
      },
    ],
  },
  {
    group: "Actions",
    items: [
      {
        icon: Add01Icon,
        shortcut: "F",
        title: "New FIR",
        url: "/dataset/new",
      },
      {
        icon: Add01Icon,
        shortcut: "N",
        title: "New template",
        url: "/templates/new",
      },
    ],
  },
];

export function SidebarCommandMenu() {
  const navigate = useNavigate();
  const [open, setOpen] = React.useState(false);

  React.useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key.toLowerCase() !== "k" || (!event.metaKey && !event.ctrlKey)) {
        return;
      }

      event.preventDefault();
      setOpen((value) => !value);
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  function runCommand(url: string) {
    setOpen(false);
    void navigate({ to: url });
  }

  return (
    <>
      <SidebarGroup>
        <SidebarGroupContent>
          <InputGroup
            className="cursor-pointer bg-background shadow-none group-data-[collapsible=icon]:hidden"
            onClick={() => setOpen(true)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                setOpen(true);
              }
            }}
            role="button"
            tabIndex={0}
          >
            <InputGroupAddon align="inline-start">
              <HugeiconsIcon icon={Search01Icon} />
            </InputGroupAddon>
            <InputGroupInput
              aria-label="Search commands"
              className="pointer-events-none"
              placeholder="Search actions"
              readOnly
            />
            <InputGroupAddon align="inline-end">
              <KbdGroup>
                <Kbd>Ctrl</Kbd>
                <Kbd>K</Kbd>
              </KbdGroup>
            </InputGroupAddon>
          </InputGroup>
        </SidebarGroupContent>
      </SidebarGroup>

      <CommandDialog open={open} onOpenChange={setOpen}>
        <Command>
          <CommandInput placeholder="Search actions..." />
          <CommandList>
            <CommandEmpty>No actions found.</CommandEmpty>
            {commandActions.map((section, index) => (
              <React.Fragment key={section.group}>
                {index > 0 ? <CommandSeparator /> : null}
                <CommandGroup heading={section.group}>
                  {section.items.map((item) => (
                    <CommandItem key={item.url} onSelect={() => runCommand(item.url)}>
                      <HugeiconsIcon icon={item.icon} />
                      <span>{item.title}</span>
                      <CommandShortcut>{item.shortcut}</CommandShortcut>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </React.Fragment>
            ))}
          </CommandList>
        </Command>
      </CommandDialog>
    </>
  );
}

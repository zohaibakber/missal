import type { ComponentProps } from "react";

import { useKeyboardShortcuts } from "#/components/keyboard-shortcuts";
import { NavMain } from "#/components/nav-main";
import { NavProjects } from "#/components/nav-projects";
import { ShortcutKbd, shortcutLabel } from "#/components/shortcut-kbd";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#/components/ui/sidebar";
import { KeyboardIcon, Settings01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Link, useRouterState } from "@tanstack/react-router";

export function AppSidebar(props: ComponentProps<typeof Sidebar>) {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const { openShortcuts } = useKeyboardShortcuts();

  return (
    <Sidebar
      className="top-(--header-height) h-[calc(100svh-var(--header-height))]!"
      collapsible="offcanvas"
      variant="sidebar"
      {...props}
    >
      <SidebarContent>
        <NavMain />
        <NavProjects />
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton onClick={openShortcuts} tooltip="Keyboard shortcuts">
              <HugeiconsIcon icon={KeyboardIcon} strokeWidth={2} />
              <span>Keyboard shortcuts</span>
            </SidebarMenuButton>
            <SidebarMenuBadge>
              <ShortcutKbd id="cheatsheet" />
            </SidebarMenuBadge>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={pathname === "/settings"}
              render={<Link to="/settings" />}
              tooltip="Settings"
              title={shortcutLabel("goSettings", "Settings")}
            >
              <HugeiconsIcon icon={Settings01Icon} strokeWidth={2} />
              <span>Settings</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}

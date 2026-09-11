import type { ComponentProps } from "react";

import { NavMain } from "#/components/nav-main";
import { NavProjects } from "#/components/nav-projects";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#/components/ui/sidebar";
import { Settings01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Link, useRouterState } from "@tanstack/react-router";

export function AppSidebar(props: ComponentProps<typeof Sidebar>) {
  const { location } = useRouterState();

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
            <SidebarMenuButton
              className={location.pathname === "/settings" ? "bg-secondary border" : ""}
              render={<Link to="/settings" />}
              tooltip="Settings"
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

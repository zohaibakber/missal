import * as React from "react";

import { NavMain } from "#/components/nav-main";
import { NavProjects } from "#/components/nav-projects";
import { NavSecondary } from "#/components/nav-secondary";
import { NavUser } from "#/components/nav-user";
import { SidebarCommandMenu } from "#/components/sidebar-command-menu";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarTrigger,
  useSidebar,
} from "#/components/ui/sidebar";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Settings05Icon,
  ChartRingIcon,
  SentIcon,
  CommandIcon,
  Files,
  Home,
} from "@hugeicons/core-free-icons";

const data = {
  navMain: [
    {
      title: "Home",
      url: "/",
      icon: <HugeiconsIcon icon={Home} strokeWidth={2} />,
      isActive: true,
    },
    {
      title: "Templates",
      url: "/templates",
      icon: <HugeiconsIcon icon={Files} strokeWidth={2} />,
    },
    {
      title: "Settings",
      url: "/settings",
      icon: <HugeiconsIcon icon={Settings05Icon} strokeWidth={2} />,
    },
  ],
  navSecondary: [
    {
      title: "Support",
      url: "#",
      icon: <HugeiconsIcon icon={ChartRingIcon} strokeWidth={2} />,
    },
    {
      title: "Feedback",
      url: "#",
      icon: <HugeiconsIcon icon={SentIcon} strokeWidth={2} />,
    },
  ],
};

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { state } = useSidebar();
  return (
    <Sidebar variant="sidebar" collapsible="icon" {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" render={<a href="#" />}>
              {state == "expanded" && (
                <>
                  <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                    <HugeiconsIcon icon={CommandIcon} strokeWidth={2} className="size-4" />
                  </div>
                  <div className="grid flex-1 text-start text-sm leading-tight">
                    <span className="truncate font-medium">Missal</span>
                    <span className="truncate text-xs">Writing</span>
                  </div>
                </>
              )}
              <SidebarTrigger />
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <SidebarCommandMenu />
        <NavMain items={data.navMain} />
        <NavProjects />
        <NavSecondary items={data.navSecondary} className="mt-auto" />
      </SidebarContent>
      <SidebarFooter>
        <NavUser />
      </SidebarFooter>
    </Sidebar>
  );
}

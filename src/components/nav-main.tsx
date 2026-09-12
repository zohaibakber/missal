import {
  SidebarGroup,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#/components/ui/sidebar";
import { HugeiconsIcon } from "@hugeicons/react";
import { Files, Home, TextFontIcon } from "@hugeicons/core-free-icons";
import { Link, useRouterState } from "@tanstack/react-router";
const links = [
  { to: "/", label: "Home", icon: Home },
  { to: "/templates", label: "Templates", icon: Files },
  { to: "/placeholders", label: "Placeholders", icon: TextFontIcon },
] as const;

export function NavMain() {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  return (
    <SidebarGroup>
      <SidebarMenu>
        {links.map((link) => (
          <SidebarMenuItem key={link.to}>
            <SidebarMenuButton
              render={<Link to={link.to} />}
              tooltip={link.label}
              isActive={link.to === "/" ? pathname === "/" : pathname.startsWith(link.to)}
            >
              <HugeiconsIcon icon={link.icon} strokeWidth={2} />
              <span>{link.label}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        ))}
      </SidebarMenu>
    </SidebarGroup>
  );
}

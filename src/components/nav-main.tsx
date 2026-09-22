import {
  SidebarGroup,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#/components/ui/sidebar";
import { shortcutLabel } from "#/components/shortcut-kbd";
import type { ShortcutId } from "#/lib/shortcuts";
import { HugeiconsIcon } from "@hugeicons/react";
import { Files, Home, TextFontIcon } from "@hugeicons/core-free-icons";
import { Link, useRouterState } from "@tanstack/react-router";

const links = [
  { to: "/", label: "FIRs", icon: Home, shortcut: "goHome" },
  { to: "/templates", label: "Templates", icon: Files, shortcut: "goTemplates" },
  { to: "/placeholders", label: "Placeholders", icon: TextFontIcon, shortcut: "goPlaceholders" },
] as const satisfies readonly { to: string; label: string; icon: unknown; shortcut: ShortcutId }[];

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
              title={shortcutLabel(link.shortcut, link.label)}
              isActive={
                link.to === "/"
                  ? pathname === "/" || pathname === "/new" || /^\/\d+/.test(pathname)
                  : pathname.startsWith(link.to)
              }
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

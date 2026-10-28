import type { ComponentProps } from "react";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/reactivity";
import { Link, useRouterState } from "@tanstack/react-router";
import {
  Add01Icon,
  Folder01Icon,
  KeyboardIcon,
  LegalDocument01Icon,
  Settings01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { FirStatusIcon } from "#/components/fir-status-badge";
import { useKeyboardShortcuts } from "#/components/keyboard-shortcuts";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
} from "#/components/ui/sidebar";
import { atoms } from "#/state/atoms";

function usePathname() {
  return useRouterState({ select: (state) => state.location.pathname });
}

const isFirPath = (pathname: string) => pathname === "/" || /^\/\d+/.test(pathname);

function useCount(result: AsyncResult.AsyncResult<readonly unknown[], unknown>) {
  return AsyncResult.isSuccess(result) && result.value.length ? result.value.length : null;
}

function NewFirButton() {
  const pathname = usePathname();
  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <SidebarMenuButton
          variant="outline"
          isActive={pathname === "/new"}
          render={<Link to="/new" />}
          tooltip="New FIR"
        >
          <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
          <span>New FIR</span>
        </SidebarMenuButton>
        <SidebarMenuBadge>
          <ShortcutKbd id="newFir" />
        </SidebarMenuBadge>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}

function NavMain() {
  const pathname = usePathname();
  const firCount = useCount(useAtomValue(atoms.firsAtom));
  const templateCount = useCount(useAtomValue(atoms.templatesAtom));
  const links = [
    {
      to: "/",
      label: "FIRs",
      icon: Folder01Icon,
      active: isFirPath(pathname),
      count: firCount,
    },
    {
      to: "/templates",
      label: "Templates",
      icon: LegalDocument01Icon,
      active: pathname.startsWith("/templates"),
      count: templateCount,
    },
  ] as const;

  return (
    <SidebarGroup>
      <SidebarMenu className="gap-px">
        {links.map((link) => (
          <SidebarMenuItem key={link.to}>
            <SidebarMenuButton
              render={<Link to={link.to} />}
              tooltip={link.label}
              isActive={link.active}
            >
              <HugeiconsIcon icon={link.icon} strokeWidth={2} />
              <span>{link.label}</span>
            </SidebarMenuButton>
            {link.count ? <SidebarMenuBadge>{link.count}</SidebarMenuBadge> : null}
          </SidebarMenuItem>
        ))}
      </SidebarMenu>
    </SidebarGroup>
  );
}

function RecentFirs() {
  const firs = useAtomValue(atoms.latestFirsAtom);
  const pathname = usePathname();

  if (AsyncResult.isFailure(firs)) return null;
  if (AsyncResult.isSuccess(firs) && !firs.value.length) return null;

  return (
    <SidebarGroup className="group-data-[collapsible=icon]:hidden">
      <SidebarGroupLabel>Recent</SidebarGroupLabel>
      <SidebarMenu className="gap-px">
        {AsyncResult.isSuccess(firs)
          ? firs.value.map((fir) => (
              <SidebarMenuItem key={fir.id}>
                <SidebarMenuButton
                  render={<Link to="/$firId" params={{ firId: `${fir.id}` }} />}
                  isActive={pathname === `/${fir.id}` || pathname === `/${fir.id}/edit`}
                >
                  <FirStatusIcon status={fir.status} />
                  <span className="shrink-0 tabular-nums">{fir.fir_no}</span>
                  <span lang="ur" dir="rtl" className="min-w-0 flex-1 text-ur-sm opacity-60">
                    {fir.offence}
                  </span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            ))
          : Array.from({ length: 3 }, (_, index) => (
              <SidebarMenuItem key={index}>
                <SidebarMenuSkeleton />
              </SidebarMenuItem>
            ))}
      </SidebarMenu>
    </SidebarGroup>
  );
}

function NavSecondary() {
  const pathname = usePathname();
  const { openShortcuts } = useKeyboardShortcuts();

  return (
    <SidebarMenu className="gap-px">
      <SidebarMenuItem>
        <SidebarMenuButton onClick={openShortcuts} tooltip="Keyboard shortcuts">
          <HugeiconsIcon icon={KeyboardIcon} strokeWidth={2} />
          <span>Shortcuts</span>
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
        >
          <HugeiconsIcon icon={Settings01Icon} strokeWidth={2} />
          <span>Settings</span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}

export function AppSidebar(props: ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar
      className="top-(--titlebar-height) h-[calc(100svh-var(--titlebar-height))]!"
      collapsible="icon"
      variant="inset"
      {...props}
    >
      <SidebarHeader>
        <NewFirButton />
      </SidebarHeader>
      <SidebarContent>
        <NavMain />
        <RecentFirs />
      </SidebarContent>
      <SidebarFooter>
        <NavSecondary />
      </SidebarFooter>
    </Sidebar>
  );
}

"use client";

import { useMemo } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { ClientOnly, Link, useRouterState } from "@tanstack/react-router";
import { File01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#/components/ui/sidebar";
import { firCollection } from "#/db-collections";

const RECENT_FIR_LIMIT = 5;

function EmptyRecentFirs() {
  return (
    <SidebarMenuItem>
      <SidebarMenuButton disabled>
        <HugeiconsIcon icon={File01Icon} strokeWidth={2} />
        <span>No FIRs yet</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  );
}

function RecentFirMenuItems() {
  const { data: firRecords } = useLiveQuery(firCollection);
  const { location } = useRouterState();
  const recentFirs = useMemo(
    () => [...firRecords].sort((first, second) => second.id - first.id).slice(0, RECENT_FIR_LIMIT),
    [firRecords],
  );

  if (!recentFirs.length) {
    return <EmptyRecentFirs />;
  }

  return recentFirs.map((fir) => {
    const url = `/${fir.id}`;
    return (
      <SidebarMenuItem key={fir.id}>
        <SidebarMenuButton
          render={<Link to="/$firId" params={{ firId: `${fir.id}` }} />}
          className={location.pathname === url ? "bg-secondary border" : ""}
          tooltip={`FIR ${fir.fir_no}`}
        >
          <HugeiconsIcon icon={File01Icon} strokeWidth={2} />
          <span>FIR {fir.fir_no}</span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    );
  });
}

export function NavProjects() {
  return (
    <SidebarGroup className="group-data-[collapsible=icon]:hidden">
      <SidebarGroupLabel>Recent FIRs</SidebarGroupLabel>
      <SidebarMenu>
        <ClientOnly fallback={<EmptyRecentFirs />}>
          <RecentFirMenuItems />
        </ClientOnly>
      </SidebarMenu>
    </SidebarGroup>
  );
}

import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Link, useRouterState } from "@tanstack/react-router";
import { File01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#/components/ui/sidebar";
import { atoms } from "#/state/atoms";

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
  const firs = useAtomValue(atoms.latestFirsAtom);
  const { location } = useRouterState();

  if (AsyncResult.isInitial(firs) || AsyncResult.isWaiting(firs)) {
    return null;
  }

  const recentFirs = AsyncResult.isSuccess(firs) ? firs.value : [];

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
        <RecentFirMenuItems />
      </SidebarMenu>
    </SidebarGroup>
  );
}

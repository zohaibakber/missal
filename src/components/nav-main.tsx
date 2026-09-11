import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "#/components/ui/collapsible";
import {
  SidebarGroup,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from "#/components/ui/sidebar";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowRight01Icon, Files, Home, TextFontIcon } from "@hugeicons/core-free-icons";
import { Link, useRouterState } from "@tanstack/react-router";
import { atoms } from "#/state/atoms";

function LatestTemplateSubItems() {
  const templates = useAtomValue(atoms.latestTemplatesAtom);
  const { location } = useRouterState();
  const latestTemplates = AsyncResult.isSuccess(templates) ? templates.value : [];

  if (
    AsyncResult.isInitial(templates) ||
    AsyncResult.isWaiting(templates) ||
    !latestTemplates.length
  ) {
    return null;
  }

  return latestTemplates.map((template) => {
    const url = `/templates/${template.id}`;
    return (
      <SidebarMenuSubItem key={template.id}>
        <SidebarMenuSubButton
          render={<Link to="/templates/$templateId" params={{ templateId: `${template.id}` }} />}
          className={location.pathname === url ? "bg-secondary border" : ""}
        >
          <span>{template.name}</span>
        </SidebarMenuSubButton>
      </SidebarMenuSubItem>
    );
  });
}

function TemplatesNavDropdown() {
  const templates = useAtomValue(atoms.templatesAtom);

  if (!AsyncResult.isSuccess(templates) || !templates.value.length) {
    return null;
  }

  return (
    <>
      <CollapsibleTrigger render={<SidebarMenuAction className="aria-expanded:rotate-90" />}>
        <HugeiconsIcon icon={ArrowRight01Icon} strokeWidth={2} />
        <span className="sr-only">Toggle</span>
      </CollapsibleTrigger>
      <CollapsibleContent dir="rtl">
        <SidebarMenuSub>
          <LatestTemplateSubItems />
        </SidebarMenuSub>
      </CollapsibleContent>
    </>
  );
}

export function NavMain() {
  const { location } = useRouterState();

  return (
    <SidebarGroup>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton
            className={location.pathname === "/" ? "bg-secondary border" : ""}
            render={<Link to="/" />}
            tooltip="Home"
          >
            <HugeiconsIcon icon={Home} strokeWidth={2} />
            <span>Home</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
        <Collapsible render={<SidebarMenuItem />}>
          <SidebarMenuButton
            className={location.pathname.startsWith("/templates") ? "bg-secondary border" : ""}
            render={<Link to="/templates" />}
            tooltip="Templates"
          >
            <HugeiconsIcon icon={Files} strokeWidth={2} />
            <span>Templates</span>
          </SidebarMenuButton>
          <TemplatesNavDropdown />
        </Collapsible>
        <SidebarMenuItem>
          <SidebarMenuButton
            className={location.pathname === "/placeholders" ? "bg-secondary border" : ""}
            render={<Link to="/placeholders" />}
            tooltip="Placeholders"
          >
            <HugeiconsIcon icon={TextFontIcon} strokeWidth={2} />
            <span>Placeholders</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarGroup>
  );
}

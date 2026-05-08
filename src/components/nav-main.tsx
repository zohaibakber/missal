import { useMemo } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "#/components/ui/collapsible";
import {
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from "#/components/ui/sidebar";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { ClientOnly, Link, useRouterState } from "@tanstack/react-router";
import { templateCollection } from "#/db-collections";

const LATEST_TEMPLATE_LIMIT = 5;

function EmptyTemplateSubItem() {
  return (
    <SidebarMenuSubItem>
      <SidebarMenuSubButton aria-disabled="true" className="pointer-events-none opacity-50">
        <span>No templates yet</span>
      </SidebarMenuSubButton>
    </SidebarMenuSubItem>
  );
}

function LatestTemplateSubItems() {
  const { data: templates } = useLiveQuery(templateCollection);
  const { location } = useRouterState();
  const latestTemplates = useMemo(
    () =>
      [...templates]
        .sort((first, second) => {
          const updatedAtComparison =
            new Date(second.updatedAt).getTime() - new Date(first.updatedAt).getTime();
          return updatedAtComparison || second.id - first.id;
        })
        .slice(0, LATEST_TEMPLATE_LIMIT),
    [templates],
  );

  if (!latestTemplates.length) {
    return <EmptyTemplateSubItem />;
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

export function NavMain({
  items,
}: {
  items: {
    title: string;
    url: string;
    icon: React.ReactNode;
    isActive?: boolean;
    items?: {
      title: string;
      url: string;
    }[];
  }[];
}) {
  const { location } = useRouterState();
  return (
    <SidebarGroup>
      <SidebarGroupLabel>Navigation</SidebarGroupLabel>
      <SidebarMenu>
        {items.map((item) => (
          <Collapsible key={item.title} defaultOpen={item.isActive} render={<SidebarMenuItem />}>
            <SidebarMenuButton
              tooltip={item.title}
              render={<Link to={item.url} />}
              className={location.pathname === item.url ? "bg-secondary border" : ""}
            >
              {item.icon}
              <span>{item.title}</span>
            </SidebarMenuButton>
            {item.items?.length || item.url === "/templates" ? (
              <>
                <CollapsibleTrigger
                  render={<SidebarMenuAction className="aria-expanded:rotate-90" />}
                >
                  <HugeiconsIcon icon={ArrowRight01Icon} strokeWidth={2} />
                  <span className="sr-only">Toggle</span>
                </CollapsibleTrigger>
                <CollapsibleContent dir="rtl">
                  <SidebarMenuSub>
                    {item.url === "/templates" ? (
                      <ClientOnly fallback={<EmptyTemplateSubItem />}>
                        <LatestTemplateSubItems />
                      </ClientOnly>
                    ) : (
                      item.items?.map((subItem) => (
                        <SidebarMenuSubItem key={subItem.title}>
                          <SidebarMenuSubButton render={<a href={subItem.url} />}>
                            <span>{subItem.title}</span>
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>
                      ))
                    )}
                  </SidebarMenuSub>
                </CollapsibleContent>
              </>
            ) : null}
          </Collapsible>
        ))}
      </SidebarMenu>
    </SidebarGroup>
  );
}

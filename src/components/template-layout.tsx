import { useRef, useState, type ReactNode } from "react";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Link, useNavigate, useRouterState } from "@tanstack/react-router";
import { Add01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { IconAction } from "#/components/icon-action";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Badge } from "#/components/ui/badge";
import { InputGroup, InputGroupAddon, InputGroupInput } from "#/components/ui/input-group";
import {
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
} from "#/components/ui/sidebar";
import { ScrollArea } from "#/components/ui/scroll-area";
import { Empty, EmptyHeader, EmptyTitle, EmptyDescription } from "#/components/ui/empty";
import { Skeleton } from "#/components/ui/skeleton";
import { useShortcut } from "#/hooks/use-shortcut";
import { atoms } from "#/state/atoms";

export function TemplateLayout({ children }: { children: ReactNode }) {
  const result = useAtomValue(atoms.templatesAtom);
  const navigate = useNavigate();
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const [search, setSearch] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const templates = AsyncResult.isSuccess(result) ? result.value : [];
  const query = search.trim().toLocaleLowerCase();
  const filtered = templates.filter((template) =>
    `${template.name} ${template.previewText}`.toLocaleLowerCase().includes(query),
  );

  useShortcut("search", () => {
    searchRef.current?.focus();
    searchRef.current?.select();
  });

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden lg:flex-row">
      <Sidebar
        collapsible="none"
        className="h-auto max-h-64 w-full shrink-0 border-b lg:h-full lg:max-h-none lg:w-64 lg:border-e lg:border-b-0"
      >
        <SidebarHeader>
          <div className="flex items-center justify-between gap-2 ps-1">
            <span className="flex items-center gap-2 text-sm font-medium">
              Templates
              <Badge variant="secondary">{templates.length}</Badge>
            </span>
            <IconAction
              label="New template"
              shortcut="newTemplate"
              nativeButton={false}
              render={<Link to="/templates/new" />}
            >
              <HugeiconsIcon icon={Add01Icon} />
            </IconAction>
          </div>
          <InputGroup className="h-8">
            <InputGroupAddon align="inline-start">
              <HugeiconsIcon icon={Search01Icon} />
            </InputGroupAddon>
            <InputGroupInput
              ref={searchRef}
              aria-label="Search templates"
              placeholder="Search templates…"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape" && search) {
                  event.stopPropagation();
                  setSearch("");
                } else if (event.key === "ArrowDown") {
                  event.preventDefault();
                  listRef.current?.querySelector<HTMLElement>("a")?.focus();
                } else if (event.key === "Enter" && filtered[0]) {
                  event.preventDefault();
                  void navigate({
                    to: "/templates/$templateId",
                    params: { templateId: `${filtered[0].id}` },
                  });
                }
              }}
            />
            <InputGroupAddon align="inline-end">
              <ShortcutKbd id="search" />
            </InputGroupAddon>
          </InputGroup>
        </SidebarHeader>
        <SidebarContent className="overflow-hidden">
          <ScrollArea className="min-h-0 flex-1">
            {AsyncResult.isSuccess(result) ? (
              filtered.length ? (
                <SidebarMenu
                  ref={listRef}
                  className="p-2"
                  aria-label="Templates"
                  onKeyDown={(event) => {
                    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
                    const links = [...(listRef.current?.querySelectorAll<HTMLElement>("a") ?? [])];
                    const index = links.indexOf(document.activeElement as HTMLElement);
                    const next = links[index + (event.key === "ArrowDown" ? 1 : -1)];
                    event.preventDefault();
                    if (next) next.focus();
                    else if (event.key === "ArrowUp") searchRef.current?.focus();
                  }}
                >
                  {filtered.map((template) => {
                    const active = pathname === `/templates/${template.id}`;
                    return (
                      <SidebarMenuItem key={template.id}>
                        <SidebarMenuButton
                          isActive={active}
                          aria-current={active ? "page" : undefined}
                          title={template.name}
                          render={
                            <Link
                              to="/templates/$templateId"
                              params={{ templateId: String(template.id) }}
                            />
                          }
                        >
                          <span
                            lang="ur"
                            dir="rtl"
                            className="w-full truncate text-base leading-loose"
                          >
                            {template.name}
                          </span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    );
                  })}
                </SidebarMenu>
              ) : (
                <Empty size="sm">
                  <EmptyHeader>
                    <EmptyTitle>{query ? "No matches" : "No templates yet"}</EmptyTitle>
                    <EmptyDescription>
                      {query ? "Try a different name." : "Create a template to get started."}
                    </EmptyDescription>
                  </EmptyHeader>
                </Empty>
              )
            ) : AsyncResult.isFailure(result) ? (
              <Empty size="sm">
                <EmptyHeader>
                  <EmptyTitle>Could not load templates</EmptyTitle>
                </EmptyHeader>
              </Empty>
            ) : (
              <div className="flex flex-col gap-2 p-2">
                <Skeleton className="h-8" />
                <Skeleton className="h-8" />
              </div>
            )}
          </ScrollArea>
        </SidebarContent>
      </Sidebar>
      <div className="min-h-0 min-w-0 flex-1 overflow-y-auto">{children}</div>
    </div>
  );
}

import { useDeferredValue, useRef, useState } from "react";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Link, useNavigate, useRouterState } from "@tanstack/react-router";
import { Add01Icon, Cancel01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { IconAction } from "#/components/icon-action";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { SplitViewListHeader } from "#/components/split-view";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "#/components/ui/empty";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import { SidebarMenu, SidebarMenuButton, SidebarMenuItem } from "#/components/ui/sidebar";
import { Skeleton } from "#/components/ui/skeleton";
import { useShortcut } from "#/hooks/use-shortcut";
import type { TemplateSummary } from "#/lib/templates";
import { atoms } from "#/state/atoms";

const matches = (template: TemplateSummary, query: string) =>
  `${template.name} ${template.previewText}`.toLocaleLowerCase().includes(query);

export function TemplateList() {
  const result = useAtomValue(atoms.templatesAtom);
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const [search, setSearch] = useState("");
  const query = useDeferredValue(search.trim().toLocaleLowerCase());
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const templates = AsyncResult.isSuccess(result) ? result.value : [];
  const filtered = query ? templates.filter((template) => matches(template, query)) : templates;

  useShortcut("search", () => {
    searchRef.current?.focus();
    searchRef.current?.select();
  });

  function focusLink(offset: 1 | -1) {
    const links = [...(listRef.current?.querySelectorAll<HTMLElement>("a") ?? [])];
    const index = links.indexOf(document.activeElement as HTMLElement);
    const next = links[index + offset];
    if (next) next.focus();
    else if (offset === -1) searchRef.current?.focus();
  }

  return (
    <>
      <SplitViewListHeader className="ps-2">
        <InputGroup className="h-7 flex-1">
          <InputGroupAddon align="inline-start">
            <HugeiconsIcon icon={Search01Icon} strokeWidth={2} />
          </InputGroupAddon>
          <InputGroupInput
            ref={searchRef}
            aria-label="Search templates"
            placeholder="Search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape" && search) {
                event.stopPropagation();
                setSearch("");
              } else if (event.key === "ArrowDown") {
                event.preventDefault();
                focusLink(1);
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
            {search ? (
              <InputGroupButton
                aria-label="Clear search"
                size="icon-xs"
                onClick={() => {
                  setSearch("");
                  searchRef.current?.focus();
                }}
              >
                <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
              </InputGroupButton>
            ) : (
              <ShortcutKbd id="search" />
            )}
          </InputGroupAddon>
        </InputGroup>
        <IconAction
          label="New template"
          shortcut="newTemplate"
          nativeButton={false}
          render={<Link to="/templates/new" />}
        >
          <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
        </IconAction>
      </SplitViewListHeader>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {AsyncResult.isSuccess(result) ? (
          filtered.length ? (
            <SidebarMenu
              ref={listRef}
              className="gap-px p-2 pt-1"
              aria-label="Templates"
              onKeyDown={(event) => {
                if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
                event.preventDefault();
                focusLink(event.key === "ArrowDown" ? 1 : -1);
              }}
            >
              {filtered.map((template) => {
                const active = pathname === `/templates/${template.id}`;
                return (
                  <SidebarMenuItem key={template.id}>
                    <SidebarMenuButton
                      isActive={active}
                      aria-current={active ? "page" : undefined}
                      size="multiline"
                      render={
                        <Link
                          to="/templates/$templateId"
                          params={{ templateId: String(template.id) }}
                        />
                      }
                    >
                      <span lang="ur" dir="rtl" className="truncate text-ur">
                        {template.name}
                      </span>
                      <span
                        lang="ur"
                        dir="rtl"
                        className="truncate text-xs leading-6 text-muted-foreground"
                      >
                        {template.previewText || "خالی"}
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
                  {query ? "Try a different name." : "Create one or import a Word file."}
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
          <div className="flex flex-col gap-1 p-2 pt-1">
            {[0, 1, 2].map((row) => (
              <Skeleton key={row} className="h-14" />
            ))}
          </div>
        )}
      </div>
    </>
  );
}

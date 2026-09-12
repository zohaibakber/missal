import { useRef, useState, type ReactNode } from "react";
import { useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Link, useRouterState } from "@tanstack/react-router";
import { Add01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "#/components/ui/button";
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
import { atoms } from "#/state/atoms";

export function TemplateLayout({ children }: { children: ReactNode }) {
  const result = useAtomValue(atoms.templatesAtom);
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const [search, setSearch] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const searchToggleRef = useRef<HTMLButtonElement>(null);
  const templates = AsyncResult.isSuccess(result) ? result.value : [];
  const query = search.trim().toLocaleLowerCase();
  const filtered = templates.filter((template) =>
    `${template.name} ${template.previewText}`.toLocaleLowerCase().includes(query),
  );

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden lg:flex-row">
      <Sidebar
        collapsible="none"
        dir="rtl"
        className="h-auto max-h-64 w-full shrink-0 border-b lg:h-full lg:max-h-none lg:w-60 lg:border-e lg:border-b-0"
      >
        <SidebarHeader className="gap-2 p-2">
          <div className="flex items-center justify-between gap-1">
            <Button
              ref={searchToggleRef}
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label="ٹیمپلیٹس تلاش کریں"
              title="ٹیمپلیٹس تلاش کریں"
              aria-expanded={searchOpen}
              aria-controls="template-search"
              onClick={() => {
                setSearchOpen((open) => !open);
                setSearch("");
              }}
            >
              <HugeiconsIcon icon={Search01Icon} />
            </Button>
            <Button
              nativeButton={false}
              render={<Link to="/templates/new" />}
              variant="ghost"
              size="icon-sm"
              aria-label="نیا ٹیمپلیٹ"
              title="نیا ٹیمپلیٹ"
            >
              <HugeiconsIcon icon={Add01Icon} />
            </Button>
          </div>
          {searchOpen ? (
            <InputGroup>
              <InputGroupAddon align="inline-start">
                <HugeiconsIcon icon={Search01Icon} />
              </InputGroupAddon>
              <InputGroupInput
                autoFocus
                id="template-search"
                aria-label="ٹیمپلیٹ کا نام تلاش کریں"
                placeholder="تلاش کریں"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    setSearch("");
                    setSearchOpen(false);
                    searchToggleRef.current?.focus();
                  }
                }}
              />
            </InputGroup>
          ) : null}
        </SidebarHeader>
        <SidebarContent className="overflow-hidden">
          <ScrollArea className="min-h-0 flex-1" dir="rtl">
            {AsyncResult.isSuccess(result) ? (
              filtered.length ? (
                <SidebarMenu className="px-2 pb-2" aria-label="Templates">
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
                          <span lang="ur" className="truncate text-base leading-loose">
                            {template.name}
                          </span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    );
                  })}
                </SidebarMenu>
              ) : (
                <Empty className="p-4">
                  <EmptyHeader>
                    <EmptyTitle>{query ? "کوئی نتیجہ نہیں" : "ابھی کوئی ٹیمپلیٹ نہیں"}</EmptyTitle>
                    <EmptyDescription>
                      {query ? "دوسرا نام تلاش کریں۔" : "نیا ٹیمپلیٹ شامل کریں۔"}
                    </EmptyDescription>
                  </EmptyHeader>
                </Empty>
              )
            ) : AsyncResult.isFailure(result) ? (
              <Empty className="p-4">
                <EmptyHeader>
                  <EmptyTitle>ٹیمپلیٹس لوڈ نہیں ہو سکے</EmptyTitle>
                </EmptyHeader>
              </Empty>
            ) : (
              <div className="flex flex-col gap-2 p-3">
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

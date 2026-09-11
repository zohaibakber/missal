import { SidebarCommandMenu } from "#/components/sidebar-command-menu";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "#/components/ui/breadcrumb";
import { Separator } from "#/components/ui/separator";
import { SidebarTrigger } from "#/components/ui/sidebar";
import { Link, useRouterState } from "@tanstack/react-router";

function getPageLabel(pathname: string) {
  if (pathname === "/") return "FIR Dataset";
  if (pathname === "/new") return "New FIR";
  if (pathname === "/templates") return "Templates";
  if (pathname === "/templates/new") return "New template";
  if (pathname === "/placeholders") return "Placeholders";
  if (pathname === "/settings") return "Settings";
  if (pathname.endsWith("/edit")) return "Edit FIR";
  if (pathname.startsWith("/templates/")) return "Edit template";
  return "FIR details";
}

export function SiteHeader() {
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const isNestedTemplateRoute = pathname.startsWith("/templates/");

  return (
    <header className="sticky top-0 z-50 flex w-full shrink-0 items-center border-b bg-background [-webkit-app-region:drag] [&_a]:[-webkit-app-region:no-drag] [&_button]:[-webkit-app-region:no-drag]">
      <div className="flex h-(--header-height) w-full items-center gap-2 px-4 pr-[calc(100vw-env(titlebar-area-width,100vw)-env(titlebar-area-x,0px)+1rem)]">
        <SidebarTrigger className="-ml-1 size-8" />
        <Separator
          className="mr-2 data-vertical:h-4 data-vertical:self-auto"
          orientation="vertical"
        />
        <Breadcrumb className="hidden min-w-0 sm:block">
          <BreadcrumbList className="flex-nowrap">
            <BreadcrumbItem>
              <BreadcrumbLink render={<Link to="/" />}>Missal</BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
            {isNestedTemplateRoute ? (
              <>
                <BreadcrumbItem>
                  <BreadcrumbLink render={<Link to="/templates" />}>Templates</BreadcrumbLink>
                </BreadcrumbItem>
                <BreadcrumbSeparator />
              </>
            ) : null}
            <BreadcrumbItem className="min-w-0">
              <BreadcrumbPage className="truncate">{getPageLabel(pathname)}</BreadcrumbPage>
            </BreadcrumbItem>
          </BreadcrumbList>
        </Breadcrumb>
        <div className="ml-auto [-webkit-app-region:no-drag]">
          <SidebarCommandMenu />
        </div>
      </div>
    </header>
  );
}

import { Fragment } from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import { CommandMenu } from "#/components/command-menu";
import { shortcutLabel } from "#/components/shortcut-kbd";
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

type Crumb = { label: string; to?: "/" | "/templates" };

function getCrumbs(pathname: string): Crumb[] {
  if (pathname === "/") return [{ label: "FIRs" }];
  if (pathname === "/new") return [{ label: "FIRs", to: "/" }, { label: "New FIR" }];
  if (pathname === "/templates") return [{ label: "Templates" }];
  if (pathname === "/templates/new")
    return [{ label: "Templates", to: "/templates" }, { label: "New template" }];
  if (pathname.startsWith("/templates/"))
    return [{ label: "Templates", to: "/templates" }, { label: "Edit template" }];
  if (pathname === "/placeholders") return [{ label: "Placeholders" }];
  if (pathname === "/settings") return [{ label: "Settings" }];
  return [{ label: "FIRs", to: "/" }, { label: "FIR workspace" }];
}

export function SiteHeader() {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const crumbs = getCrumbs(pathname);

  return (
    <header className="sticky top-0 z-50 flex w-full shrink-0 items-center border-b bg-background [-webkit-app-region:drag] [&_a]:[-webkit-app-region:no-drag] [&_button]:[-webkit-app-region:no-drag]">
      <div className="titlebar-padding flex h-(--header-height) w-full items-center gap-2 ps-3">
        <SidebarTrigger title={shortcutLabel("toggleSidebar")} />
        <Separator
          orientation="vertical"
          className="me-1 data-vertical:h-4 data-vertical:self-auto"
        />
        <Breadcrumb className="min-w-0">
          <BreadcrumbList className="flex-nowrap">
            {crumbs.map((crumb, index) => (
              <Fragment key={crumb.label}>
                {index > 0 ? <BreadcrumbSeparator /> : null}
                <BreadcrumbItem className="min-w-0">
                  {crumb.to ? (
                    <BreadcrumbLink render={<Link to={crumb.to} />}>{crumb.label}</BreadcrumbLink>
                  ) : (
                    <BreadcrumbPage className="truncate">{crumb.label}</BreadcrumbPage>
                  )}
                </BreadcrumbItem>
              </Fragment>
            ))}
          </BreadcrumbList>
        </Breadcrumb>
        <div className="ms-auto">
          <CommandMenu />
        </div>
      </div>
    </header>
  );
}

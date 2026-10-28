import { createContext, Fragment, use, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Link, type LinkProps } from "@tanstack/react-router";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "#/components/ui/breadcrumb";

type Crumb = { label: ReactNode; to?: LinkProps["to"]; lang?: "ur" };

type Slot = { target: HTMLElement | null; setTarget: (element: HTMLElement | null) => void };

const BreadcrumbSlotContext = createContext<Slot | null>(null);

function AppBreadcrumbProvider({ children }: { children: ReactNode }) {
  const [target, setTarget] = useState<HTMLElement | null>(null);
  return <BreadcrumbSlotContext value={{ target, setTarget }}>{children}</BreadcrumbSlotContext>;
}

function AppBreadcrumbOutlet() {
  const slot = use(BreadcrumbSlotContext);
  return <div ref={slot?.setTarget} className="flex min-w-0 items-center" />;
}

function AppBreadcrumb({ crumbs }: { crumbs: readonly Crumb[] }) {
  const target = use(BreadcrumbSlotContext)?.target;
  if (!target) return null;

  return createPortal(
    <Breadcrumb className="min-w-0">
      <BreadcrumbList className="flex-nowrap">
        {crumbs.map((crumb, index) => (
          <Fragment key={index}>
            {index ? <BreadcrumbSeparator /> : null}
            <BreadcrumbItem className="min-w-0">
              {crumb.to ? (
                <BreadcrumbLink render={<Link to={crumb.to} />}>{crumb.label}</BreadcrumbLink>
              ) : (
                <BreadcrumbPage
                  lang={crumb.lang}
                  dir={crumb.lang === "ur" ? "rtl" : undefined}
                  className="truncate"
                >
                  {crumb.label}
                </BreadcrumbPage>
              )}
            </BreadcrumbItem>
          </Fragment>
        ))}
      </BreadcrumbList>
    </Breadcrumb>,
    target,
  );
}

export { AppBreadcrumb, AppBreadcrumbOutlet, AppBreadcrumbProvider };

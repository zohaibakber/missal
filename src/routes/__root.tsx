import { TemplateLayout } from "#/components/template-layout";
import { AppSidebar } from "#/components/app-sidebar";
import { SiteHeader } from "#/components/site-header";
import { ThemeProvider } from "#/components/theme-provider";
import { DirectionProvider } from "#/components/ui/direction";
import { SidebarInset, SidebarProvider } from "#/components/ui/sidebar";
import { Toaster } from "#/components/ui/toast";
import { TooltipProvider } from "#/components/ui/tooltip";
import { HeadContent, Link, Outlet, createRootRoute, useRouterState } from "@tanstack/react-router";

function NotFound() {
  return (
    <div className="flex h-dvh w-full flex-col items-center justify-center gap-2">
      <h1>Not Found</h1>
      <Link to="/" className="text-sm underline">
        Go Home
      </Link>
    </div>
  );
}

export const Route = createRootRoute({
  head: () => ({
    meta: [
      {
        charSet: "utf-8",
      },
      {
        name: "viewport",
        content: "width=device-width, initial-scale=1",
      },
      {
        title: "Missal Writing",
      },
    ],
  }),
  component: RootComponent,
  notFoundComponent: NotFound,
});

function RootComponent() {
  const isTemplates = useRouterState({
    select: (state) => state.location.pathname.startsWith("/templates"),
  });
  return (
    <>
      <HeadContent />
      <ThemeProvider>
        <TooltipProvider>
          <DirectionProvider direction="ltr">
            <SidebarProvider className="h-svh min-h-0 flex-col overflow-hidden [--header-height:calc(var(--spacing)*10)]">
              <SiteHeader />
              <div className="flex min-h-0 flex-1">
                <AppSidebar />
                <SidebarInset dir="rtl" className="min-h-0 overflow-y-auto">
                  <DirectionProvider direction="rtl">
                    {isTemplates ? (
                      <TemplateLayout>
                        <Outlet />
                      </TemplateLayout>
                    ) : (
                      <Outlet />
                    )}
                  </DirectionProvider>
                </SidebarInset>
              </div>
            </SidebarProvider>
          </DirectionProvider>
        </TooltipProvider>
        <Toaster />
      </ThemeProvider>
    </>
  );
}

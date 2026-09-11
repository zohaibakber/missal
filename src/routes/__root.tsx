import { AppSidebar } from "#/components/app-sidebar";
import { SiteHeader } from "#/components/site-header";
import { ThemeProvider } from "#/components/theme-provider";
import { DirectionProvider } from "#/components/ui/direction";
import { SidebarInset, SidebarProvider } from "#/components/ui/sidebar";
import { Toaster } from "#/components/ui/sonner";
import { TooltipProvider } from "#/components/ui/tooltip";
import { HeadContent, Link, Outlet, createRootRoute } from "@tanstack/react-router";

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
                <SidebarInset className="min-h-0 overflow-hidden max-w-3xl mx-auto">
                  <Outlet />
                </SidebarInset>
              </div>
            </SidebarProvider>
          </DirectionProvider>
        </TooltipProvider>
        <Toaster richColors />
      </ThemeProvider>
    </>
  );
}

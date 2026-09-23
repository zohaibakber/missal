import { AppSidebar } from "#/components/app-sidebar";
import { KeyboardShortcutsProvider } from "#/components/keyboard-shortcuts";
import { TitleBar } from "#/components/title-bar";
import { ThemeProvider } from "#/components/theme-provider";
import { Button } from "#/components/ui/button";
import { DirectionProvider } from "#/components/ui/direction";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "#/components/ui/empty";
import { SidebarInset, SidebarProvider } from "#/components/ui/sidebar";
import { Toaster } from "#/components/ui/toast";
import { TooltipProvider } from "#/components/ui/tooltip";
import { useAppShortcuts } from "#/hooks/use-app-shortcuts";
import { HeadContent, Link, Outlet, createRootRoute } from "@tanstack/react-router";

function NotFound() {
  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyTitle>Page not found</EmptyTitle>
        <EmptyDescription>This page doesn't exist or was moved.</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button nativeButton={false} render={<Link to="/" />} variant="outline">
          Back to FIRs
        </Button>
      </EmptyContent>
    </Empty>
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
  useAppShortcuts();

  return (
    <>
      <HeadContent />
      <ThemeProvider>
        <TooltipProvider>
          <DirectionProvider direction="ltr">
            <KeyboardShortcutsProvider>
              <SidebarProvider className="h-svh min-h-0 flex-col overflow-hidden">
                <TitleBar />
                <div className="flex min-h-0 flex-1">
                  <AppSidebar />
                  <SidebarInset className="min-h-0 overflow-hidden">
                    <Outlet />
                  </SidebarInset>
                </div>
              </SidebarProvider>
            </KeyboardShortcutsProvider>
          </DirectionProvider>
        </TooltipProvider>
        <Toaster />
      </ThemeProvider>
    </>
  );
}

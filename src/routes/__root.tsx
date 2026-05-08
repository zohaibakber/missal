import {
  HeadContent,
  Link,
  Scripts,
  createRootRouteWithContext,
  redirect,
} from "@tanstack/react-router";
import { TanStackRouterDevtoolsPanel } from "@tanstack/react-router-devtools";
import { TanStackDevtools } from "@tanstack/react-devtools";
import { ClerkProvider, Show } from "@clerk/tanstack-react-start";

import appCss from "../styles.css?url";

import { TooltipProvider } from "#/components/ui/tooltip";
import { SidebarInset, SidebarProvider } from "#/components/ui/sidebar";
import { DirectionProvider } from "#/components/ui/direction";
import { AppSidebar } from "#/components/app-sidebar";
import { createServerFn } from "@tanstack/react-start";
import { auth } from "@clerk/tanstack-react-start/server";
import { ThemeProvider } from "#/components/theme-provider";
import { ToastProvider } from "#/components/ui/toast";

function NotFound() {
  return (
    <div className="flex flex-col h-dvh items-center justify-center w-full gap-2">
      <h1>Not Found</h1>
      <Link to="/" className="text-sm underline">
        Go Home
      </Link>
    </div>
  );
}

const authStateFn = createServerFn({ method: "GET" }).handler(async () => {
  const { isAuthenticated, userId } = await auth();
  return { isAuthenticated, userId };
});

export const Route = createRootRouteWithContext()({
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
    links: [
      {
        rel: "stylesheet",
        href: appCss,
      },
    ],
  }),
  beforeLoad: async ({ location }) => {
    const { isAuthenticated, userId } = await authStateFn();
    // Only redirect to sign-in if not authenticated AND not already on sign-in page
    if (!isAuthenticated && !location.pathname.startsWith("/sign-in")) {
      throw redirect({ to: "/sign-in/$" });
    }
    return { userId };
  },
  loader: async () => {
    return {};
  },
  shellComponent: RootDocument,
  notFoundComponent: NotFound,
});

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        <ClerkProvider>
          <ThemeProvider>
            <ToastProvider>
              <TooltipProvider>
                <DirectionProvider direction="ltr">
                  <SidebarProvider>
                    <Show when={"signed-in"}>
                      <AppSidebar />
                      <SidebarInset>{children}</SidebarInset>
                    </Show>
                    <Show when={"signed-out"}>{children}</Show>
                  </SidebarProvider>
                </DirectionProvider>
              </TooltipProvider>
            </ToastProvider>
          </ThemeProvider>
        </ClerkProvider>
        <TanStackDevtools
          config={{
            position: "bottom-right",
          }}
          plugins={[
            {
              name: "Tanstack Router",
              render: <TanStackRouterDevtoolsPanel />,
            },
          ]}
        />
        <Scripts />
      </body>
    </html>
  );
}

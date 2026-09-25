import { useSyncExternalStore } from "react";
import { useRouter } from "@tanstack/react-router";
import { ArrowLeft02Icon, ArrowRight02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { CommandMenu } from "#/components/command-menu";
import { IconAction } from "#/components/icon-action";
import { SidebarTrigger } from "#/components/ui/sidebar";
import { Hint } from "#/components/hint";
import { useShortcut } from "#/hooks/use-shortcut";

type HistoryNavigation = EventTarget & { canGoBack: boolean; canGoForward: boolean };

// Chromium's Navigation API tracks same-document history, so it knows about forward entries too.
const historyNavigation = (window as { navigation?: HistoryNavigation }).navigation;

function subscribe(onChange: () => void) {
  historyNavigation?.addEventListener("currententrychange", onChange);
  return () => historyNavigation?.removeEventListener("currententrychange", onChange);
}

function useHistoryAvailability() {
  const canGoBack = useSyncExternalStore(subscribe, () => historyNavigation?.canGoBack ?? true);
  const canGoForward = useSyncExternalStore(
    subscribe,
    () => historyNavigation?.canGoForward ?? true,
  );
  return { canGoBack, canGoForward };
}

function HistoryButtons() {
  const router = useRouter();
  const { canGoBack, canGoForward } = useHistoryAvailability();

  useShortcut("goBack", () => router.history.back(), { enabled: canGoBack });
  useShortcut("goForward", () => router.history.forward(), { enabled: canGoForward });

  return (
    <div className="flex items-center">
      <IconAction
        label="Back"
        shortcut="goBack"
        disabled={!canGoBack}
        onClick={() => router.history.back()}
        variant="subtle"
      >
        <HugeiconsIcon icon={ArrowLeft02Icon} strokeWidth={2} />
      </IconAction>
      <IconAction
        label="Forward"
        shortcut="goForward"
        disabled={!canGoForward}
        onClick={() => router.history.forward()}
        variant="subtle"
      >
        <HugeiconsIcon icon={ArrowRight02Icon} strokeWidth={2} />
      </IconAction>
    </div>
  );
}

export function TitleBar() {
  return (
    <header className="grid h-(--titlebar-height) shrink-0 grid-cols-[1fr_minmax(0,28rem)_1fr] items-center gap-3 bg-sidebar app-drag [&_button]:app-no-drag [&_a]:app-no-drag">
      <div className="flex items-center gap-0.5 ps-2.5">
        <Hint label="Toggle sidebar" shortcut="toggleSidebar">
          <SidebarTrigger variant="subtle" />
        </Hint>
        <HistoryButtons />
      </div>
      <CommandMenu />
      <div className="titlebar-padding" />
    </header>
  );
}

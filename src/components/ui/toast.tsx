import { createContext, useContext, useMemo } from "react";
import type { CSSProperties, ReactNode } from "react";
import { Toast as ToastPrimitive } from "@base-ui/react/toast";
import {
  Alert01Icon,
  CancelCircleIcon,
  CheckmarkCircle02Icon,
  InformationCircleIcon,
  Loading03Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { cn } from "#/lib/utils";

type ToastContextValue = {
  success: (title: string) => void;
};

const ToastContext = createContext<ToastContextValue | null>(null);

const toastStyle = {
  "--normal-bg": "var(--popover)",
  "--normal-text": "var(--popover-foreground)",
  "--normal-border": "var(--border)",
  "--border-radius": "var(--radius)",
} as CSSProperties;

const attentionAnimations = ["animate-toast-attention-a", "animate-toast-attention-b"];

function ToastIcon({ type }: { type?: string }) {
  const icon =
    type === "error"
      ? CancelCircleIcon
      : type === "warning"
        ? Alert01Icon
        : type === "info"
          ? InformationCircleIcon
          : type === "loading"
            ? Loading03Icon
            : CheckmarkCircle02Icon;

  return (
    <HugeiconsIcon
      aria-hidden="true"
      className={cn("size-4 shrink-0", type === "loading" && "animate-spin")}
      icon={icon}
      strokeWidth={2}
    />
  );
}

function ToastViewport() {
  const { toasts } = ToastPrimitive.useToastManager();

  return (
    <ToastPrimitive.Portal>
      <ToastPrimitive.Viewport
        className="toaster group fixed end-4 bottom-4 z-50 flex w-[min(22rem,calc(100vw-2rem))] flex-col gap-2"
        style={toastStyle}
      >
        {toasts.map((toast) => (
          <ToastPrimitive.Root
            key={toast.id}
            toast={toast}
            className={cn(
              "cn-toast absolute end-0 bottom-0 z-[calc(1000-var(--toast-index))] w-full rounded-[var(--border-radius)] border border-[var(--normal-border)] bg-[var(--normal-bg)] px-3 py-2 text-sm text-[var(--normal-text)] shadow-lg outline-none transition-[opacity,transform] duration-200 ease-out",
              "translate-x-[var(--toast-swipe-movement-x)] translate-y-[calc(var(--toast-swipe-movement-y)+(var(--toast-index)*-20%))] scale-[calc(1-(0.05*var(--toast-index)))]",
              "data-[expanded]:translate-y-[calc(var(--toast-offset-y)*-1)] data-[expanded]:scale-100",
              "data-[starting-style]:translate-y-4 data-[starting-style]:opacity-0",
              "data-[ending-style]:opacity-0 data-[ending-style]:data-[swipe-direction=right]:translate-x-[calc(var(--toast-swipe-movement-x)+150%)] data-[ending-style]:data-[swipe-direction=left]:translate-x-[calc(var(--toast-swipe-movement-x)-150%)] data-[ending-style]:data-[swipe-direction=down]:translate-y-[calc(var(--toast-swipe-movement-y)+150%)] data-[ending-style]:data-[swipe-direction=up]:translate-y-[calc(var(--toast-swipe-movement-y)-150%)]",
              toast.updateKey
                ? attentionAnimations[toast.updateKey % attentionAnimations.length]
                : null,
            )}
          >
            <ToastPrimitive.Content className="flex items-center gap-2 overflow-hidden transition-opacity duration-200 data-[behind]:opacity-0 data-[expanded]:opacity-100">
              <ToastIcon type={toast.type} />
              <div className="grid gap-0.5">
                <ToastPrimitive.Title className="font-medium" />
                {toast.description ? (
                  <ToastPrimitive.Description className="text-muted-foreground text-xs" />
                ) : null}
              </div>
            </ToastPrimitive.Content>
          </ToastPrimitive.Root>
        ))}
      </ToastPrimitive.Viewport>
    </ToastPrimitive.Portal>
  );
}

function ToastContextProvider({ children }: { children: ReactNode }) {
  const manager = ToastPrimitive.useToastManager();
  const value = useMemo<ToastContextValue>(
    () => ({
      success: (title) => {
        manager.add({
          id: `success:${title}`,
          title,
          type: "success",
        });
      },
    }),
    [manager],
  );

  return (
    <ToastContext value={value}>
      {children}
      <ToastViewport />
    </ToastContext>
  );
}

export function ToastProvider({ children }: { children: ReactNode }) {
  return (
    <ToastPrimitive.Provider limit={3} timeout={3000}>
      <ToastContextProvider>{children}</ToastContextProvider>
    </ToastPrimitive.Provider>
  );
}

export function useToast() {
  const context = useContext(ToastContext);

  if (!context) {
    throw new Error("useToast must be used within ToastProvider");
  }

  return context;
}

import type { ReactNode } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { KeyboardIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useKeyboardShortcuts } from "#/components/keyboard-shortcuts";
import { Pane, PaneBody, PaneHeader, PaneTitle } from "#/components/pane";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { useTheme } from "#/components/theme-provider";
import { Button } from "#/components/ui/button";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSeparator,
  FieldSet,
  FieldTitle,
} from "#/components/ui/field";
import { RadioGroup, RadioGroupItem } from "#/components/ui/radio-group";
import type { DesktopTheme } from "#/desktop-window";

export const Route = createFileRoute("/settings")({
  component: RouteComponent,
});

const items = [
  { label: "System", value: "system" },
  { label: "Light", value: "light" },
  { label: "Dark", value: "dark" },
] as const;

function RouteComponent() {
  const { theme, setTheme } = useTheme();
  const { openShortcuts } = useKeyboardShortcuts();

  return (
    <Pane>
      <PaneHeader>
        <PaneTitle>Settings</PaneTitle>
      </PaneHeader>
      <PaneBody>
        <FieldGroup className="mx-auto w-full max-w-2xl px-6 py-8">
          <FieldSet>
            <FieldLegend>Appearance</FieldLegend>
            <FieldDescription>System follows your Windows light or dark setting.</FieldDescription>
            <RadioGroup
              className="flex w-auto flex-row gap-4 pt-2"
              name="theme"
              onValueChange={(value) => {
                if (value === "system" || value === "light" || value === "dark") {
                  setTheme(value);
                }
              }}
              value={theme}
            >
              {items.map((item) => (
                <Field key={item.value} className="w-auto">
                  <FieldLabel
                    appearance="plain"
                    className="relative flex-col"
                    htmlFor={`theme-${item.value}`}
                  >
                    <RadioGroupItem
                      className="peer sr-only absolute"
                      id={`theme-${item.value}`}
                      value={item.value}
                    />
                    <span className="relative block h-[70px] w-[88px] overflow-hidden rounded-md ring-1 ring-foreground/10 transition-shadow peer-focus-visible:ring-3 peer-focus-visible:ring-ring/50 peer-data-checked:ring-2 peer-data-checked:ring-primary peer-data-checked:ring-offset-2 peer-data-checked:ring-offset-background">
                      {themePreviews[item.value]}
                    </span>
                    <span className="text-xs not-peer-data-checked:text-muted-foreground">
                      {item.label}
                    </span>
                  </FieldLabel>
                </Field>
              ))}
            </RadioGroup>
          </FieldSet>
          <FieldSeparator />
          <Field orientation="horizontal">
            <FieldContent>
              <FieldTitle>Keyboard shortcuts</FieldTitle>
              <FieldDescription>
                Press <ShortcutKbd id="commandMenu" /> to search commands and{" "}
                <ShortcutKbd id="cheatsheet" /> to see every shortcut.
              </FieldDescription>
            </FieldContent>
            <Button variant="outline" size="sm" onClick={openShortcuts}>
              <HugeiconsIcon icon={KeyboardIcon} strokeWidth={2} data-icon="inline-start" />
              Show all
            </Button>
          </Field>
        </FieldGroup>
      </PaneBody>
    </Pane>
  );
}

const themePreviews: Record<DesktopTheme, ReactNode> = {
  dark: (
    <svg
      aria-hidden
      className="size-full"
      fill="none"
      viewBox="0 0 88 70"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path className="fill-theme-preview-900" d="M0 0h88v70H0z" />
      <path className="fill-theme-preview-800 shadow-sm" d="M10 12a4 4 0 0 1 4-4h74v62H10V12Z" />
      <circle className="fill-theme-preview-600" cx="28" cy="26" r="8" />
      <rect className="fill-theme-preview-700" height="4" rx="2" width="58" x="20" y="42" />
      <rect className="fill-theme-preview-700" height="4" rx="2" width="58" x="20" y="49" />
      <rect className="fill-theme-preview-700" height="4" rx="2" width="29" x="20" y="56" />
    </svg>
  ),
  light: (
    <svg
      aria-hidden
      className="size-full"
      fill="none"
      viewBox="0 0 88 70"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path className="fill-theme-preview-200" d="M0 0h88v70H0z" />
      <path className="fill-white shadow-sm" d="M10 12a4 4 0 0 1 4-4h74v62H10V12Z" />
      <circle className="fill-theme-preview-300" cx="28" cy="26" r="8" />
      <rect className="fill-theme-preview-200" height="4" rx="2" width="58" x="20" y="42" />
      <rect className="fill-theme-preview-200" height="4" rx="2" width="58" x="20" y="49" />
      <rect className="fill-theme-preview-200" height="4" rx="2" width="29" x="20" y="56" />
    </svg>
  ),
  system: (
    <svg
      aria-hidden
      className="size-full"
      fill="none"
      viewBox="0 0 88 70"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path className="fill-theme-preview-200" d="M0 0h44v70H0z" />
      <path className="fill-theme-preview-900" d="M44 0h44v70H44z" />
      <path className="fill-white shadow-sm" d="M10 12a4 4 0 0 1 4-4h30v62H10V12Z" />
      <circle className="fill-theme-preview-300" cx="28" cy="26" r="8" />
      <path
        className="fill-theme-preview-200"
        d="M20 44a2 2 0 0 1 2-2h22v4H22a2 2 0 0 1-2-2ZM20 51a2 2 0 0 1 2-2h22v4H22a2 2 0 0 1-2-2ZM20 58a2 2 0 0 1 2-2h22v4H22a2 2 0 0 1-2-2Z"
      />
      <path className="fill-theme-preview-800 shadow-sm" d="M54 12a4 4 0 0 1 4-4h30v62H54V12Z" />
      <circle className="fill-theme-preview-600" cx="72" cy="26" r="8" />
      <path
        className="fill-theme-preview-700"
        d="M64 44a2 2 0 0 1 2-2h22v4H66a2 2 0 0 1-2-2ZM64 51a2 2 0 0 1 2-2h22v4H66a2 2 0 0 1-2-2ZM64 58a2 2 0 0 1 2-2h22v4H66a2 2 0 0 1-2-2Z"
      />
    </svg>
  ),
};

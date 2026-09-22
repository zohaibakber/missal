import type { ReactNode } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { useTheme } from "#/components/theme-provider";
import { KeyboardIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useKeyboardShortcuts } from "#/components/keyboard-shortcuts";
import { Page, PageDescription, PageHeader, PageHeading, PageTitle } from "#/components/page";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Button } from "#/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "#/components/ui/card";
import { Field, FieldLabel, FieldLegend, FieldSet } from "#/components/ui/field";
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
    <Page width="narrow">
      <PageHeader>
        <PageHeading>
          <PageTitle>Settings</PageTitle>
          <PageDescription>Preferences are saved on this computer.</PageDescription>
        </PageHeading>
      </PageHeader>
      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
          <CardDescription>
            Choose how Missal looks. System follows your OS setting.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <FieldSet>
            <FieldLegend className="sr-only">Theme</FieldLegend>
            <RadioGroup
              className="flex w-auto flex-row gap-4"
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
                    className="relative cursor-pointer flex-col"
                    htmlFor={`theme-${item.value}`}
                  >
                    <RadioGroupItem
                      className="peer sr-only absolute"
                      id={`theme-${item.value}`}
                      value={item.value}
                    />
                    <span className="relative block h-[70px] w-[88px] overflow-hidden rounded-lg shadow-xs ring-1 ring-foreground/10 transition-shadow peer-focus-visible:ring-3 peer-focus-visible:ring-ring/50 peer-data-checked:ring-2 peer-data-checked:ring-primary peer-data-checked:ring-offset-2 peer-data-checked:ring-offset-card">
                      {themePreviews[item.value]}
                    </span>
                    <span className="text-sm not-peer-data-checked:text-muted-foreground">
                      {item.label}
                    </span>
                  </FieldLabel>
                </Field>
              ))}
            </RadioGroup>
          </FieldSet>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Keyboard</CardTitle>
          <CardDescription>
            Every action has a shortcut. Press <ShortcutKbd id="commandMenu" /> to search commands
            or <ShortcutKbd id="cheatsheet" /> to see them all.
          </CardDescription>
          <CardAction>
            <Button variant="outline" size="sm" onClick={openShortcuts}>
              <HugeiconsIcon icon={KeyboardIcon} data-icon="inline-start" />
              View shortcuts
            </Button>
          </CardAction>
        </CardHeader>
      </Card>
    </Page>
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

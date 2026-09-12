import type { ReactNode } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { useTheme } from "#/components/theme-provider";
import { Field, FieldLabel, FieldLegend, FieldSet } from "#/components/ui/field";
import { RadioGroup, RadioGroupItem } from "#/components/ui/radio-group";
import type { DesktopTheme } from "#/desktop-window";

export const Route = createFileRoute("/settings")({
  component: RouteComponent,
});

const items = [
  { label: "سسٹم", value: "system" },
  { label: "روشن", value: "light" },
  { label: "تاریک", value: "dark" },
] as const;

function RouteComponent() {
  const { theme, setTheme } = useTheme();

  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-6 p-4">
      <FieldSet className="gap-4">
        <FieldLegend className="text-sm font-medium">ظاہری انداز</FieldLegend>
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
                className="relative cursor-pointer flex-col has-data-checked:border-transparent has-data-checked:bg-transparent dark:has-data-checked:border-transparent dark:has-data-checked:bg-transparent"
                htmlFor={`theme-${item.value}`}
              >
                <RadioGroupItem
                  className="peer sr-only absolute"
                  id={`theme-${item.value}`}
                  value={item.value}
                />
                <span className="relative block h-[70px] w-[88px] overflow-hidden rounded-lg shadow-xs transition-shadow not-peer-data-checked:opacity-80 peer-data-checked:ring-2 peer-data-checked:ring-primary/48 peer-data-checked:ring-offset-1 peer-data-checked:ring-offset-background">
                  {themePreviews[item.value]}
                </span>
                <span className="not-peer-data-checked:text-muted-foreground/70">{item.label}</span>
              </FieldLabel>
            </Field>
          ))}
        </RadioGroup>
      </FieldSet>
    </main>
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
      <path className="fill-neutral-900" d="M0 0h88v70H0z" />
      <path className="fill-neutral-800 shadow-sm" d="M10 12a4 4 0 0 1 4-4h74v62H10V12Z" />
      <circle className="fill-neutral-600" cx="28" cy="26" r="8" />
      <rect className="fill-neutral-700" height="4" rx="2" width="58" x="20" y="42" />
      <rect className="fill-neutral-700" height="4" rx="2" width="58" x="20" y="49" />
      <rect className="fill-neutral-700" height="4" rx="2" width="29" x="20" y="56" />
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
      <path className="fill-neutral-200" d="M0 0h88v70H0z" />
      <path className="fill-white shadow-sm" d="M10 12a4 4 0 0 1 4-4h74v62H10V12Z" />
      <circle className="fill-neutral-300" cx="28" cy="26" r="8" />
      <rect className="fill-neutral-200" height="4" rx="2" width="58" x="20" y="42" />
      <rect className="fill-neutral-200" height="4" rx="2" width="58" x="20" y="49" />
      <rect className="fill-neutral-200" height="4" rx="2" width="29" x="20" y="56" />
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
      <path className="fill-neutral-200" d="M0 0h44v70H0z" />
      <path className="fill-neutral-900" d="M44 0h44v70H44z" />
      <path className="fill-white shadow-sm" d="M10 12a4 4 0 0 1 4-4h30v62H10V12Z" />
      <circle className="fill-neutral-300" cx="28" cy="26" r="8" />
      <path
        className="fill-neutral-200"
        d="M20 44a2 2 0 0 1 2-2h22v4H22a2 2 0 0 1-2-2ZM20 51a2 2 0 0 1 2-2h22v4H22a2 2 0 0 1-2-2ZM20 58a2 2 0 0 1 2-2h22v4H22a2 2 0 0 1-2-2Z"
      />
      <path className="fill-neutral-800 shadow-sm" d="M54 12a4 4 0 0 1 4-4h30v62H54V12Z" />
      <circle className="fill-neutral-600" cx="72" cy="26" r="8" />
      <path
        className="fill-neutral-700"
        d="M64 44a2 2 0 0 1 2-2h22v4H66a2 2 0 0 1-2-2ZM64 51a2 2 0 0 1 2-2h22v4H66a2 2 0 0 1-2-2ZM64 58a2 2 0 0 1 2-2h22v4H66a2 2 0 0 1-2-2Z"
      />
    </svg>
  ),
};

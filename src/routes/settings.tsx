import { useState, type ReactNode } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/reactivity";
import { Exit, Schema } from "effect";
import { createFileRoute } from "@tanstack/react-router";
import { FirFieldsSettings } from "#/components/fir-fields-form";
import { GlobalPlaceholderForm } from "#/components/global-placeholder-form";
import { AppBreadcrumb } from "#/components/app-breadcrumb";
import { Pane, PaneBody } from "#/components/pane";
import { SettingsSection } from "#/components/settings-section";
import { useTheme } from "#/components/theme-provider";
import { Button } from "#/components/ui/button";
import { Field, FieldError, FieldLabel } from "#/components/ui/field";
import { InputGroup, InputGroupInput, InputGroupText } from "#/components/ui/input-group";
import { RadioGroup, RadioGroupItem } from "#/components/ui/radio-group";
import { toast } from "#/components/ui/toast";
import type { DesktopTheme } from "#/desktop-window";
import { type AppSettings, FieldMarkers } from "#/lib/settings";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { whilePending } from "#/lib/utils";
import { atoms } from "#/state/atoms";

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

  return (
    <Pane>
      <AppBreadcrumb crumbs={[{ label: "Settings" }]} />
      <PaneBody>
        <div className="@container mx-auto w-full max-w-4xl px-8 py-10">
          <SettingsSection title="Theme">
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
          </SettingsSection>
          <FieldMarkersSetting />
          <GlobalPlaceholderForm />
          <FirFieldsSettings />
        </div>
      </PaneBody>
    </Pane>
  );
}

function FieldMarkersSetting() {
  const result = useAtomValue(atoms.settingsAtom);
  if (!AsyncResult.isSuccess(result)) return null;
  const { open, close } = result.value.fieldMarkers;
  return <FieldMarkersForm key={`${open}${close}`} settings={result.value} />;
}

function FieldMarkersForm({ settings }: { settings: AppSettings }) {
  const [open, setOpen] = useState(settings.fieldMarkers.open);
  const [close, setClose] = useState(settings.fieldMarkers.close);
  const [saving, setSaving] = useState(false);
  const save = useAtomSet(atoms.saveFieldMarkersAtom, { mode: "promiseExit" });
  const decoded = Schema.decodeUnknownExit(FieldMarkers)({ open, close });
  const invalid = Exit.isFailure(decoded);
  const dirty = open !== settings.fieldMarkers.open || close !== settings.fieldMarkers.close;

  async function submit() {
    if (saving || !dirty || Exit.isFailure(decoded)) return;
    const markers = decoded.value;
    const exit = await whilePending(setSaving, () => save(markers));
    if (Exit.isFailure(exit)) {
      toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
      return;
    }
    toast.add({ title: "Placeholder markers saved", type: "success" });
  }

  return (
    <SettingsSection
      title="Field markers"
      description={
        <>
          Type a name between them to insert a field, like{" "}
          <code dir="rtl" lang="ur" className="rounded bg-muted px-1.5 text-foreground">
            {open}
            جرم
            {close}
          </code>
        </>
      }
    >
      <form
        className="flex items-start gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Field data-invalid={invalid} className="w-48">
          <FieldLabel className="sr-only" htmlFor="field-marker-open">
            Field markers
          </FieldLabel>
          <InputGroup dir="rtl">
            <InputGroupInput
              id="field-marker-open"
              aria-label="Before name"
              className="text-center"
              maxLength={3}
              value={open}
              disabled={saving}
              aria-invalid={invalid}
              onChange={(event) => setOpen(event.target.value.trim())}
            />
            <InputGroupText lang="ur">نام</InputGroupText>
            <InputGroupInput
              aria-label="After name"
              className="text-center"
              maxLength={3}
              value={close}
              disabled={saving}
              aria-invalid={invalid}
              onChange={(event) => setClose(event.target.value.trim())}
            />
          </InputGroup>
          {invalid ? <FieldError>Use 1–3 symbols, no letters or numbers.</FieldError> : null}
        </Field>
        {dirty ? (
          <Button type="submit" size="sm" className="mt-0.5" disabled={invalid || saving}>
            Save
          </Button>
        ) : null}
      </form>
    </SettingsSection>
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

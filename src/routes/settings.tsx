import { useEffect, useId, useState } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { Button } from "#/components/ui/button";
import { Card, CardContent, CardHeader } from "#/components/ui/card";
import { Field, FieldGroup, FieldLabel } from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import { Skeleton } from "#/components/ui/skeleton";
import { appSettingsCollection, getAppSettings, saveAppSettings } from "#/db-collections";
import { SHARED_PLACEHOLDER_FIELDS } from "#/lib/settings";

export const Route = createFileRoute("/settings")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <ClientOnly fallback={<SettingsSkeleton />}>
      <SettingsForm />
    </ClientOnly>
  );
}

function SettingsForm() {
  const formId = useId();
  const { data: settingsRecords } = useLiveQuery(appSettingsCollection);
  const settings = getAppSettings(settingsRecords);
  const [sharedPlaceholders, setSharedPlaceholders] = useState(settings.sharedPlaceholders);

  useEffect(() => {
    setSharedPlaceholders(settings.sharedPlaceholders);
  }, [settings.sharedPlaceholders]);

  function handleSubmit() {
    saveAppSettings({
      id: "default",
      sharedPlaceholders,
      updatedAt: new Date().toISOString(),
    });
    toast.success("Settings saved");
  }

  return (
    <main className="flex flex-col gap-4 p-4">
      <section className="flex flex-col gap-1">
        <h1 className="text-xl font-medium">Settings</h1>
      </section>

      <form
        id={formId}
        className="grid gap-4"
        dir="rtl"
        onSubmit={(event) => {
          event.preventDefault();
          handleSubmit();
        }}
      >
        <FieldGroup className="grid gap-3 md:grid-cols-2">
          {SHARED_PLACEHOLDER_FIELDS.map((field) => (
            <Field key={field.key}>
              <FieldLabel htmlFor={`${formId}-${field.key}`}>{field.label}</FieldLabel>
              <Input
                id={`${formId}-${field.key}`}
                name={field.key}
                onChange={(event) =>
                  setSharedPlaceholders((current) => ({
                    ...current,
                    [field.key]: event.target.value,
                  }))
                }
                placeholder={field.placeholder}
                value={sharedPlaceholders[field.key] ?? ""}
              />
            </Field>
          ))}
        </FieldGroup>
        <div className="flex justify-end" dir="ltr">
          <Button type="submit">Save</Button>
        </div>
      </form>
    </main>
  );
}

function SettingsSkeleton() {
  return (
    <main className="flex flex-col gap-4 p-4 lg:p-6">
      <div className="flex flex-col gap-2">
        <Skeleton className="h-7 w-28" />
      </div>
      <Card className="max-w-4xl">
        <CardHeader>
          <Skeleton className="h-5 w-40" />
          <Skeleton className="h-4 w-80" />
        </CardHeader>
        <CardContent>
          <div className="grid gap-3 md:grid-cols-2">
            {SHARED_PLACEHOLDER_FIELDS.map((field) => (
              <Skeleton key={field.key} className="h-16" />
            ))}
          </div>
        </CardContent>
      </Card>
    </main>
  );
}

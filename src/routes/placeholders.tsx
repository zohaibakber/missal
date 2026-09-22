import { GlobalPlaceholderForm } from "#/components/global-placeholder-form";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "#/components/ui/tabs";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit, Schema } from "effect";
import { useForm } from "@tanstack/react-form";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "#/components/ui/toast";
import { Add01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { Button } from "#/components/ui/button";
import { Field, FieldError, FieldLabel } from "#/components/ui/field";
import {
  Page,
  PageDescription,
  PageFooter,
  PageHeader,
  PageHeading,
  PageTitle,
} from "#/components/page";
import {
  PlaceholderList,
  PlaceholderListFooter,
  PlaceholderListCell,
  PlaceholderListRow,
  PlaceholderListTable,
} from "#/components/placeholder-list";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { useShortcut } from "#/hooks/use-shortcut";
import { useState } from "react";
import { Input } from "#/components/ui/input";
import { Skeleton } from "#/components/ui/skeleton";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "#/components/ui/empty";
import { Placeholder, PlaceholderCreateInput, PlaceholderUpdateInput } from "#/lib/placeholder";
import { NonEmptyTrimmedString } from "#/lib/schema";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { atoms } from "#/state/atoms";

export const Route = createFileRoute("/placeholders")({ component: RouteComponent });

type PlaceholderTab = "global" | "fir";

function RouteComponent() {
  const result = useAtomValue(atoms.placeholdersAtom);
  const [tab, setTab] = useState<PlaceholderTab>("global");

  return (
    <Page width="narrow" className="pb-0">
      <PageHeader>
        <PageHeading>
          <PageTitle>Placeholders</PageTitle>
          <PageDescription>
            Names you insert into templates. Documents replace them with real values.
          </PageDescription>
        </PageHeading>
      </PageHeader>
      {AsyncResult.isSuccess(result) ? (
        <Tabs
          value={tab}
          onValueChange={(value) => {
            if (value === "global" || value === "fir") setTab(value);
          }}
          className="gap-4"
        >
          <TabsList>
            <TabsTrigger value="global">Global values</TabsTrigger>
            <TabsTrigger value="fir">FIR fields</TabsTrigger>
          </TabsList>
          <TabsContent value="global" keepMounted>
            <GlobalPlaceholderForm active={tab === "global"} />
          </TabsContent>
          <TabsContent value="fir" keepMounted>
            <PlaceholderPage
              active={tab === "fir"}
              placeholders={result.value.filter((field) => field.source._tag !== "SharedSetting")}
            />
          </TabsContent>
        </Tabs>
      ) : AsyncResult.isFailure(result) ? (
        <Empty variant="outline">
          <EmptyHeader>
            <EmptyTitle>Could not load placeholders</EmptyTitle>
            <EmptyDescription>
              The database could not be read. Restart Missal and try again.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <div className="flex flex-col gap-4">
          <Skeleton className="h-8 w-56" />
          <Skeleton className="h-72" />
        </div>
      )}
    </Page>
  );
}

function sourceDescription(placeholder: Placeholder | undefined) {
  if (!placeholder) return "Entered per FIR";
  return placeholder.source._tag === "FirProperty"
    ? `FIR · ${placeholder.source.property}`
    : "Entered per FIR";
}

function PlaceholderPage({
  active,
  placeholders,
}: {
  active: boolean;
  placeholders: readonly Placeholder[];
}) {
  const create = useAtomSet(atoms.createPlaceholderAtom, { mode: "promiseExit" });
  const update = useAtomSet(atoms.updatePlaceholderAtom, { mode: "promiseExit" });
  const form = useForm({
    defaultValues: {
      placeholders: placeholders.map((item) => ({ id: item.id, key: item.key, label: item.label })),
      additions: [] as { label: string }[],
    },
    onSubmit: async ({ value }) => {
      // Reset only after all writes succeed. Keep each successful row's identity on a partial failure.
      for (const item of value.placeholders) {
        if (placeholders.find((original) => original.id === item.id)?.label === item.label)
          continue;
        const decoded = Schema.decodeUnknownExit(PlaceholderUpdateInput)(item);
        if (Exit.isFailure(decoded)) return;
        const exit = await update(decoded.value);
        if (Exit.isFailure(exit)) {
          toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
          return;
        }
      }
      while (form.getFieldValue("additions").length) {
        const item = form.getFieldValue("additions")[0];
        const decoded = Schema.decodeUnknownExit(PlaceholderCreateInput)(item);
        if (Exit.isFailure(decoded)) return;
        const exit = await create(decoded.value);
        if (Exit.isFailure(exit)) {
          toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
          return;
        }
        form.pushFieldValue("placeholders", {
          id: exit.value.id,
          key: exit.value.key,
          label: exit.value.label,
        });
        await form.removeFieldValue("additions", 0);
      }
      form.reset({ placeholders: form.getFieldValue("placeholders"), additions: [] });
      toast.add({ title: "Placeholders saved", type: "success" });
    },
  });
  const validateLabel = ({ value }: { value: string }) =>
    Exit.isFailure(Schema.decodeUnknownExit(NonEmptyTrimmedString)(value))
      ? "Enter a name"
      : undefined;

  const originals = new Map(placeholders.map((placeholder) => [placeholder.id, placeholder]));

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <UnsavedChanges isDirty={() => form.state.isDirty} />
      <form.Subscribe selector={(state) => state.isDirty && !state.isSubmitting}>
        {(canSave) => (
          <SaveShortcut enabled={active && canSave} onSave={() => void form.handleSubmit()} />
        )}
      </form.Subscribe>
      <p className="text-sm text-muted-foreground">
        These are filled from each FIR's details. Renaming keeps existing templates linked.
      </p>
      <PlaceholderList>
        <PlaceholderListTable nameHeading="Name" valueHeading="Filled from">
          <form.Field name="placeholders" mode="array">
            {(arrayField) =>
              arrayField.state.value.map((item, index) => (
                <form.Field
                  key={item.id}
                  name={`placeholders[${index}].label`}
                  validators={{ onBlur: validateLabel, onSubmit: validateLabel }}
                >
                  {(field) => {
                    const invalid = field.state.meta.isTouched && !field.state.meta.isValid;
                    return (
                      <PlaceholderListRow>
                        <PlaceholderListCell>
                          <Field data-invalid={invalid}>
                            <FieldLabel className="sr-only" htmlFor={`placeholder-${item.id}`}>
                              {originals.get(item.id)?.label ?? item.label}
                            </FieldLabel>
                            <Input
                              id={`placeholder-${item.id}`}
                              name={field.name}
                              lang="ur"
                              dir="rtl"
                              value={field.state.value}
                              onBlur={field.handleBlur}
                              onChange={(event) => field.handleChange(event.target.value)}
                              aria-invalid={invalid}
                            />
                            <FieldError errors={field.state.meta.errors} />
                          </Field>
                        </PlaceholderListCell>
                        <PlaceholderListCell>
                          <span className="truncate font-mono text-xs text-muted-foreground">
                            {sourceDescription(originals.get(item.id))}
                          </span>
                        </PlaceholderListCell>
                        <PlaceholderListCell />
                      </PlaceholderListRow>
                    );
                  }}
                </form.Field>
              ))
            }
          </form.Field>
          <form.Field name="additions" mode="array">
            {(arrayField) => (
              <>
                {arrayField.state.value.map((_, index) => (
                  <form.Field
                    key={index}
                    name={`additions[${index}].label`}
                    validators={{ onBlur: validateLabel, onSubmit: validateLabel }}
                  >
                    {(field) => {
                      const invalid = field.state.meta.isTouched && !field.state.meta.isValid;
                      return (
                        <PlaceholderListRow>
                          <PlaceholderListCell>
                            <Field data-invalid={invalid}>
                              <FieldLabel className="sr-only" htmlFor={`new-placeholder-${index}`}>
                                New placeholder {index + 1}
                              </FieldLabel>
                              <Input
                                id={`new-placeholder-${index}`}
                                name={field.name}
                                lang="ur"
                                dir="rtl"
                                value={field.state.value}
                                placeholder="متغیر کا نام"
                                onBlur={field.handleBlur}
                                onChange={(event) => field.handleChange(event.target.value)}
                                aria-invalid={invalid}
                              />
                              <FieldError errors={field.state.meta.errors} />
                            </Field>
                          </PlaceholderListCell>
                          <PlaceholderListCell>
                            <span className="font-mono text-xs text-muted-foreground">
                              Entered per FIR
                            </span>
                          </PlaceholderListCell>
                          <PlaceholderListCell>
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon-sm"
                              title="Remove"
                              onClick={() => arrayField.removeValue(index)}
                              aria-label={`Remove new placeholder ${index + 1}`}
                            >
                              <HugeiconsIcon icon={Cancel01Icon} />
                            </Button>
                          </PlaceholderListCell>
                        </PlaceholderListRow>
                      );
                    }}
                  </form.Field>
                ))}
              </>
            )}
          </form.Field>
        </PlaceholderListTable>
        <form.Field name="additions" mode="array">
          {(arrayField) => (
            <PlaceholderListFooter>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => {
                  const index = arrayField.state.value.length;
                  arrayField.pushValue({ label: "" });
                  requestAnimationFrame(() =>
                    document.getElementById(`new-placeholder-${index}`)?.focus(),
                  );
                }}
              >
                <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
                Add FIR field
              </Button>
            </PlaceholderListFooter>
          )}
        </form.Field>
      </PlaceholderList>
      <form.Subscribe
        selector={(state) => ({ isDirty: state.isDirty, isSubmitting: state.isSubmitting })}
      >
        {({ isDirty, isSubmitting }) => (
          <PageFooter>
            <span className="me-auto text-xs text-muted-foreground" role="status">
              {isDirty ? "Unsaved changes" : null}
            </span>
            <Button
              type="button"
              variant="ghost"
              disabled={!isDirty || isSubmitting}
              onClick={() =>
                form.reset({
                  placeholders: placeholders.map((item) => ({
                    id: item.id,
                    key: item.key,
                    label: item.label,
                  })),
                  additions: [],
                })
              }
            >
              Reset
            </Button>
            <Button type="submit" disabled={!isDirty || isSubmitting}>
              {isSubmitting ? "Saving…" : "Save changes"}
              <ShortcutKbd id="save" />
            </Button>
          </PageFooter>
        )}
      </form.Subscribe>
    </form>
  );
}

function SaveShortcut({ enabled, onSave }: { enabled: boolean; onSave: () => void }) {
  useShortcut("save", onSave, { enabled });
  return null;
}

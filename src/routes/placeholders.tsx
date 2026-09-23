import { useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit, Schema } from "effect";
import { useForm } from "@tanstack/react-form";
import { createFileRoute } from "@tanstack/react-router";
import { Add01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { GlobalPlaceholderForm } from "#/components/global-placeholder-form";
import { Hint } from "#/components/hint";
import {
  Pane,
  PaneActionsOutlet,
  PaneActionsPortal,
  PaneActionsProvider,
  PaneBody,
  PaneHeader,
  PaneTitle,
  SaveStatus,
} from "#/components/pane";
import {
  PlaceholderList,
  PlaceholderListCell,
  PlaceholderListFooter,
  PlaceholderListIntro,
  PlaceholderListRow,
  PlaceholderListTable,
} from "#/components/placeholder-list";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { Button } from "#/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "#/components/ui/empty";
import { Field, FieldError, FieldLabel } from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import { Skeleton } from "#/components/ui/skeleton";
import { Spinner } from "#/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "#/components/ui/tabs";
import { toast } from "#/components/ui/toast";
import { useShortcut } from "#/hooks/use-shortcut";
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
    <PaneActionsProvider>
      <Tabs
        value={tab}
        onValueChange={(value) => {
          if (value === "global" || value === "fir") setTab(value);
        }}
        className="h-full"
      >
        <Pane>
          <PaneHeader>
            <PaneTitle>Placeholders</PaneTitle>
            <TabsList className="ms-3">
              <TabsTrigger value="global">Global values</TabsTrigger>
              <TabsTrigger value="fir">FIR fields</TabsTrigger>
            </TabsList>
            <PaneActionsOutlet />
          </PaneHeader>
          <PaneBody>
            <div className="mx-auto w-full max-w-3xl px-6 py-8">
              {AsyncResult.isSuccess(result) ? (
                <>
                  <TabsContent value="global" keepMounted>
                    <GlobalPlaceholderForm active={tab === "global"} />
                  </TabsContent>
                  <TabsContent value="fir" keepMounted>
                    <FirFieldsForm
                      active={tab === "fir"}
                      placeholders={result.value.filter(
                        (field) => field.source._tag !== "SharedSetting",
                      )}
                    />
                  </TabsContent>
                </>
              ) : AsyncResult.isFailure(result) ? (
                <Empty>
                  <EmptyHeader>
                    <EmptyTitle>Could not load placeholders</EmptyTitle>
                    <EmptyDescription>
                      The database could not be read. Restart Missal.
                    </EmptyDescription>
                  </EmptyHeader>
                </Empty>
              ) : (
                <div className="flex flex-col gap-3">
                  <Skeleton className="h-4 w-80" />
                  <Skeleton className="h-64" />
                </div>
              )}
            </div>
          </PaneBody>
        </Pane>
      </Tabs>
    </PaneActionsProvider>
  );
}

function sourceDescription(placeholder: Placeholder | undefined) {
  if (placeholder?.source._tag === "FirProperty") return `FIR · ${placeholder.source.property}`;
  return "Entered per FIR";
}

const FIR_FIELDS_FORM_ID = "fir-fields-form";

function FirFieldsForm({
  active,
  placeholders,
}: {
  active: boolean;
  placeholders: readonly Placeholder[];
}) {
  const create = useAtomSet(atoms.createPlaceholderAtom, { mode: "promiseExit" });
  const update = useAtomSet(atoms.updatePlaceholderAtom, { mode: "promiseExit" });
  const initialValues = () => ({
    placeholders: placeholders.map((item) => ({ id: item.id, key: item.key, label: item.label })),
    additions: [] as { label: string }[],
  });
  const form = useForm({
    defaultValues: initialValues(),
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
      id={FIR_FIELDS_FORM_ID}
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <UnsavedChanges isDirty={() => form.state.isDirty} />
      <form.Subscribe
        selector={(state) => ({ isDirty: state.isDirty, isSubmitting: state.isSubmitting })}
      >
        {({ isDirty, isSubmitting }) => (
          <>
            <SaveShortcut
              enabled={active && isDirty && !isSubmitting}
              onSave={() => void form.handleSubmit()}
            />
            {active ? (
              <PaneActionsPortal>
                {isDirty ? <SaveStatus dirty /> : null}
                <Button
                  type="button"
                  variant="subtle"
                  size="sm"
                  disabled={!isDirty || isSubmitting}
                  onClick={() => form.reset(initialValues())}
                >
                  Reset
                </Button>
                <Hint label="Save" shortcut="save">
                  <Button
                    type="submit"
                    form={FIR_FIELDS_FORM_ID}
                    size="sm"
                    disabled={!isDirty || isSubmitting}
                  >
                    {isSubmitting ? <Spinner data-icon="inline-start" /> : null}
                    Save
                  </Button>
                </Hint>
              </PaneActionsPortal>
            ) : null}
          </>
        )}
      </form.Subscribe>
      <PlaceholderListIntro>
        Filled in from each FIR's details. Renaming one keeps it linked in every template.
      </PlaceholderListIntro>
      <PlaceholderList>
        <PlaceholderListTable nameHeading="نام" valueHeading="ماخذ">
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
                              variant="cell"
                              className="-ms-2.25"
                              name={field.name}
                              value={field.state.value}
                              onBlur={field.handleBlur}
                              onChange={(event) => field.handleChange(event.target.value)}
                              aria-invalid={invalid}
                            />
                            <FieldError errors={field.state.meta.errors} />
                          </Field>
                        </PlaceholderListCell>
                        <PlaceholderListCell>
                          <span dir="ltr" lang="en" className="text-xs text-muted-foreground">
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
            {(arrayField) =>
              arrayField.state.value.map((_, index) => (
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
                              variant="cell"
                              className="-ms-2.25"
                              name={field.name}
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
                          <span dir="ltr" lang="en" className="text-xs text-muted-foreground">
                            Entered per FIR
                          </span>
                        </PlaceholderListCell>
                        <PlaceholderListCell>
                          <Button
                            type="button"
                            variant="subtle"
                            size="icon-xs"
                            onClick={() => arrayField.removeValue(index)}
                            aria-label={`Remove new placeholder ${index + 1}`}
                          >
                            <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
                          </Button>
                        </PlaceholderListCell>
                      </PlaceholderListRow>
                    );
                  }}
                </form.Field>
              ))
            }
          </form.Field>
        </PlaceholderListTable>
        <form.Field name="additions" mode="array">
          {(arrayField) => (
            <PlaceholderListFooter>
              <Button
                type="button"
                variant="subtle"
                size="sm"
                onClick={() => {
                  const index = arrayField.state.value.length;
                  arrayField.pushValue({ label: "" });
                  requestAnimationFrame(() =>
                    document.getElementById(`new-placeholder-${index}`)?.focus(),
                  );
                }}
              >
                <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
                Add FIR field
              </Button>
            </PlaceholderListFooter>
          )}
        </form.Field>
      </PlaceholderList>
    </form>
  );
}

function SaveShortcut({ enabled, onSave }: { enabled: boolean; onSave: () => void }) {
  useShortcut("save", onSave, { enabled });
  return null;
}

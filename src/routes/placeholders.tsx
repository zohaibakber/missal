import { GlobalPlaceholderForm } from "#/components/global-placeholder-form";
import { DirectionProvider } from "#/components/ui/direction";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "#/components/ui/tabs";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit, Schema } from "effect";
import { useForm } from "@tanstack/react-form";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "#/components/ui/toast";
import { Add01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { Button } from "#/components/ui/button";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldDescription,
  FieldSet,
  FieldLegend,
} from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import { Skeleton } from "#/components/ui/skeleton";
import { Empty, EmptyHeader, EmptyTitle } from "#/components/ui/empty";
import { Placeholder, PlaceholderCreateInput, PlaceholderUpdateInput } from "#/lib/placeholder";
import { NonEmptyTrimmedString } from "#/lib/schema";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { atoms } from "#/state/atoms";

export const Route = createFileRoute("/placeholders")({ component: RouteComponent });

function RouteComponent() {
  const result = useAtomValue(atoms.placeholdersAtom);
  if (AsyncResult.isSuccess(result))
    return (
      <DirectionProvider direction="ltr">
        <main className="mx-auto flex w-full max-w-3xl flex-col gap-4 p-4" dir="ltr">
          <h1 className="text-base font-medium">Placeholders</h1>
          <Tabs defaultValue="global" className="gap-5">
            <TabsList variant="line">
              <TabsTrigger value="global">Global values</TabsTrigger>
              <TabsTrigger value="fir">FIR fields</TabsTrigger>
            </TabsList>
            <TabsContent value="global" keepMounted>
              <GlobalPlaceholderForm />
            </TabsContent>
            <TabsContent value="fir" keepMounted>
              <PlaceholderPage
                placeholders={result.value.filter((field) => field.source._tag !== "SharedSetting")}
              />
            </TabsContent>
          </Tabs>
        </main>
      </DirectionProvider>
    );
  if (AsyncResult.isFailure(result))
    return (
      <Empty>
        <EmptyHeader>
          <EmptyTitle>متغیرات لوڈ نہیں ہو سکے</EmptyTitle>
        </EmptyHeader>
      </Empty>
    );
  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-3 p-6">
      <Skeleton className="h-10" />
      <Skeleton className="h-72" />
    </div>
  );
}

function PlaceholderPage({ placeholders }: { placeholders: readonly Placeholder[] }) {
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
      toast.add({ title: "تبدیلیاں محفوظ ہو گئیں", type: "success" });
    },
  });
  const validateLabel = ({ value }: { value: string }) =>
    Exit.isFailure(Schema.decodeUnknownExit(NonEmptyTrimmedString)(value))
      ? "نام درج کریں"
      : undefined;

  return (
    <div dir="rtl">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <UnsavedChanges isDirty={() => form.state.isDirty} />
        <FieldSet className="gap-5">
          <FieldLegend className="sr-only">متغیرات</FieldLegend>
          <FieldDescription>
            یہ متغیرات ہر ایف آئی آر کی معلومات سے پُر ہوتے ہیں۔ نام بدلنے سے ٹیمپلیٹ کے روابط برقرار
            رہیں گے۔
          </FieldDescription>
          <form.Field name="placeholders" mode="array">
            {(arrayField) => (
              <FieldGroup className="gap-3">
                {arrayField.state.value.map((item, index) => (
                  <form.Field
                    key={item.id}
                    name={`placeholders[${index}].label`}
                    validators={{ onBlur: validateLabel, onSubmit: validateLabel }}
                  >
                    {(field) => (
                      <Field data-invalid={field.state.meta.isTouched && !field.state.meta.isValid}>
                        <FieldLabel className="sr-only" htmlFor={`placeholder-${item.id}`}>
                          {placeholders.find((p) => p.id === item.id)?.label ?? item.label}
                        </FieldLabel>
                        <Input
                          id={`placeholder-${item.id}`}
                          name={field.name}
                          value={field.state.value}
                          onBlur={field.handleBlur}
                          onChange={(event) => field.handleChange(event.target.value)}
                          aria-invalid={field.state.meta.isTouched && !field.state.meta.isValid}
                        />
                        <FieldError errors={field.state.meta.errors} />
                      </Field>
                    )}
                  </form.Field>
                ))}
              </FieldGroup>
            )}
          </form.Field>
          <form.Field name="additions" mode="array">
            {(arrayField) => (
              <FieldGroup className="gap-3">
                {arrayField.state.value.map((_, index) => (
                  <form.Field
                    key={index}
                    name={`additions[${index}].label`}
                    validators={{ onBlur: validateLabel, onSubmit: validateLabel }}
                  >
                    {(field) => (
                      <Field data-invalid={field.state.meta.isTouched && !field.state.meta.isValid}>
                        <FieldLabel className="sr-only" htmlFor={`new-placeholder-${index}`}>
                          نیا متغیر {index + 1}
                        </FieldLabel>
                        <div className="flex items-center gap-2">
                          <Input
                            id={`new-placeholder-${index}`}
                            name={field.name}
                            value={field.state.value}
                            placeholder="متغیر کا نام"
                            onBlur={field.handleBlur}
                            onChange={(event) => field.handleChange(event.target.value)}
                            aria-invalid={field.state.meta.isTouched && !field.state.meta.isValid}
                          />
                          <Button
                            type="button"
                            variant="ghost"
                            onClick={() => arrayField.removeValue(index)}
                            aria-label={`نیا متغیر ${index + 1} ہٹائیں`}
                          >
                            ہٹائیں
                          </Button>
                        </div>
                        <FieldError errors={field.state.meta.errors} />
                      </Field>
                    )}
                  </form.Field>
                ))}
                <Button
                  type="button"
                  variant="outline"
                  className="self-start"
                  onClick={() => {
                    const index = arrayField.state.value.length;
                    arrayField.pushValue({ label: "" });
                    requestAnimationFrame(() =>
                      document.getElementById(`new-placeholder-${index}`)?.focus(),
                    );
                  }}
                >
                  <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
                  متغیر شامل کریں
                </Button>
              </FieldGroup>
            )}
          </form.Field>
          <form.Subscribe
            selector={(state) => ({ isDirty: state.isDirty, isSubmitting: state.isSubmitting })}
          >
            {({ isDirty, isSubmitting }) => (
              <div className="sticky bottom-0 flex gap-2 border-t bg-background py-3">
                <Button type="submit" disabled={!isDirty || isSubmitting}>
                  {isSubmitting ? "محفوظ ہو رہا ہے…" : "تبدیلیاں محفوظ کریں"}
                </Button>
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
                  واپس کریں
                </Button>
              </div>
            )}
          </form.Subscribe>
        </FieldSet>
      </form>
    </div>
  );
}

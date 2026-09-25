import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit, Schema } from "effect";
import { useForm } from "@tanstack/react-form";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  PlaceholderList,
  PlaceholderListActions,
  PlaceholderToken,
} from "#/components/placeholder-list";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "#/components/ui/empty";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import { Skeleton } from "#/components/ui/skeleton";
import { toast } from "#/components/ui/toast";
import { useShortcut } from "#/hooks/use-shortcut";
import {
  Placeholder,
  PlaceholderCreateInput,
  PlaceholderName,
  PlaceholderUpdateInput,
} from "#/lib/placeholder";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { atoms } from "#/state/atoms";

export function FirFieldsSettings() {
  const result = useAtomValue(atoms.placeholdersAtom);
  if (AsyncResult.isSuccess(result)) {
    return (
      <FirFieldsForm
        placeholders={result.value.filter((field) => field.source._tag !== "SharedSetting")}
      />
    );
  }
  if (AsyncResult.isFailure(result)) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyTitle>Could not load placeholders</EmptyTitle>
          <EmptyDescription>The database could not be read. Restart Missal.</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }
  return (
    <div className="flex flex-col gap-3">
      <Skeleton className="h-4 w-80" />
      <Skeleton className="h-40" />
    </div>
  );
}

function FirFieldsForm({ placeholders }: { placeholders: readonly Placeholder[] }) {
  const create = useAtomSet(atoms.createPlaceholderAtom, { mode: "promiseExit" });
  const update = useAtomSet(atoms.updatePlaceholderAtom, { mode: "promiseExit" });
  const initialValues = () => ({
    placeholders: placeholders.map((item) => ({ id: item.id, label: item.label })),
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
        form.pushFieldValue("placeholders", { id: exit.value.id, label: exit.value.label });
        await form.removeFieldValue("additions", 0);
      }
      form.reset({ placeholders: form.getFieldValue("placeholders"), additions: [] });
      toast.add({ title: "Placeholders saved", type: "success" });
    },
  });
  const validateLabel = ({ value }: { value: string }) =>
    Exit.isFailure(Schema.decodeUnknownExit(PlaceholderName)(value)) ? "Enter a name" : undefined;

  const originals = new Map(placeholders.map((placeholder) => [placeholder.id, placeholder]));

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <UnsavedChanges isDirty={() => form.state.isDirty} />
      <FieldSet>
        <FieldLegend>FIR fields</FieldLegend>
        <FieldDescription>
          Filled in from each FIR's details. Write a name in a template like{" "}
          <PlaceholderToken name="جرم" />. Renaming one keeps it linked in every template.
        </FieldDescription>
        <PlaceholderList>
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
                      <Field data-invalid={invalid}>
                        <FieldLabel className="sr-only" htmlFor={`placeholder-${item.id}`}>
                          {originals.get(item.id)?.label ?? item.label}
                        </FieldLabel>
                        <Input
                          id={`placeholder-${item.id}`}
                          name={field.name}
                          value={field.state.value}
                          onBlur={field.handleBlur}
                          onChange={(event) => field.handleChange(event.target.value)}
                          aria-invalid={invalid}
                        />
                        <FieldError errors={field.state.meta.errors} />
                      </Field>
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
                      <Field data-invalid={invalid}>
                        <FieldLabel className="sr-only" htmlFor={`new-placeholder-${index}`}>
                          New placeholder {index + 1}
                        </FieldLabel>
                        <InputGroup>
                          <InputGroupInput
                            id={`new-placeholder-${index}`}
                            name={field.name}
                            value={field.state.value}
                            placeholder="نام"
                            onBlur={field.handleBlur}
                            onChange={(event) => field.handleChange(event.target.value)}
                            aria-invalid={invalid}
                          />
                          <InputGroupAddon align="inline-end">
                            <InputGroupButton
                              size="icon-xs"
                              onClick={() => arrayField.removeValue(index)}
                              aria-label={`Remove new placeholder ${index + 1}`}
                            >
                              <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
                            </InputGroupButton>
                          </InputGroupAddon>
                        </InputGroup>
                        <FieldError errors={field.state.meta.errors} />
                      </Field>
                    );
                  }}
                </form.Field>
              ))
            }
          </form.Field>
        </PlaceholderList>
        <form.Subscribe
          selector={(state) => ({ isDirty: state.isDirty, isSubmitting: state.isSubmitting })}
        >
          {({ isDirty, isSubmitting }) => (
            <>
              <SaveShortcut
                enabled={isDirty && !isSubmitting}
                onSave={() => void form.handleSubmit()}
              />
              <PlaceholderListActions
                addLabel="Add FIR field"
                dirty={isDirty}
                saving={isSubmitting}
                onReset={() => form.reset(initialValues())}
                onAdd={() => {
                  const index = form.getFieldValue("additions").length;
                  form.pushFieldValue("additions", { label: "" });
                  requestAnimationFrame(() =>
                    document.getElementById(`new-placeholder-${index}`)?.focus(),
                  );
                }}
              />
            </>
          )}
        </form.Subscribe>
      </FieldSet>
    </form>
  );
}

function SaveShortcut({ enabled, onSave }: { enabled: boolean; onSave: () => void }) {
  useShortcut("save", onSave, { enabled });
  return null;
}

import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Cause, Exit, Match, Option, Schema } from "effect";
import { useForm } from "@tanstack/react-form";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { Button } from "#/components/ui/button";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import { Skeleton } from "#/components/ui/skeleton";
import {
  Placeholder,
  PlaceholderCreateInput,
  PlaceholderKey,
  PlaceholderUpdateInput,
} from "#/lib/placeholder";
import { NonEmptyTrimmedString } from "#/lib/schema";
import { atoms } from "#/state/atoms";

export const Route = createFileRoute("/placeholders")({
  component: RouteComponent,
});

function RouteComponent() {
  const placeholders = useAtomValue(atoms.placeholdersAtom);

  if (AsyncResult.isSuccess(placeholders)) {
    return <PlaceholderPage placeholders={placeholders.value} />;
  }

  if (AsyncResult.isFailure(placeholders)) {
    return <PlaceholdersUnavailable />;
  }

  return <PlaceholdersSkeleton />;
}

function schemaError(schema: Schema.Codec<string>, message: string) {
  return ({ value }: { value: string }) =>
    Exit.isFailure(Schema.decodeUnknownExit(schema)(value)) ? message : undefined;
}

function AddPlaceholderForm() {
  const createPlaceholder = useAtomSet(atoms.createPlaceholderAtom, { mode: "promiseExit" });
  const form = useForm({
    defaultValues: {
      key: "",
      label: "",
    },
    onSubmit: async ({ value }) => {
      const decoded = Schema.decodeUnknownExit(PlaceholderCreateInput)(value);

      if (Exit.isFailure(decoded)) {
        return;
      }

      const exit = await createPlaceholder(decoded.value);

      if (Exit.isFailure(exit)) {
        toast.error(
          Option.match(Cause.findErrorOption(exit.cause), {
            onNone: () => "Something went wrong while saving",
            onSome: (error) =>
              Match.valueTags(error, {
                EntityNotFound: ({ entity }) => `${entity} not found`,
                EntityConflict: ({ field }) => `${field} is already in use`,
                StorageError: ({ message }) => message,
              }),
          }),
        );
        return;
      }

      form.reset();
    },
  });

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <FieldGroup className="grid gap-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] md:items-end">
        <form.Field
          name="key"
          validators={{
            onBlur: schemaError(
              PlaceholderKey,
              "Use a letter, then letters, numbers, or underscores",
            ),
            onSubmit: schemaError(
              PlaceholderKey,
              "Use a letter, then letters, numbers, or underscores",
            ),
          }}
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;

            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={field.name}>Key</FieldLabel>
                <Input
                  aria-invalid={isInvalid}
                  id={field.name}
                  name={field.name}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  placeholder="police_station"
                  value={field.state.value}
                />
                {isInvalid ? <FieldError errors={field.state.meta.errors} /> : null}
              </Field>
            );
          }}
        />
        <form.Field
          name="label"
          validators={{
            onBlur: schemaError(NonEmptyTrimmedString, "Required"),
            onSubmit: schemaError(NonEmptyTrimmedString, "Required"),
          }}
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;

            return (
              <Field data-invalid={isInvalid} dir="rtl">
                <FieldLabel htmlFor={field.name}>Label</FieldLabel>
                <Input
                  aria-invalid={isInvalid}
                  dir="rtl"
                  id={field.name}
                  lang="ur"
                  name={field.name}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  placeholder="تھانہ نام"
                  value={field.state.value}
                />
                {isInvalid ? <FieldError errors={field.state.meta.errors} /> : null}
              </Field>
            );
          }}
        />
        <form.Subscribe
          selector={(state) => ({
            canSubmit: state.canSubmit,
            isSubmitting: state.isSubmitting,
          })}
          children={({ canSubmit, isSubmitting }) => (
            <Button disabled={!canSubmit} type="submit">
              {isSubmitting ? "Adding..." : "Add"}
            </Button>
          )}
        />
      </FieldGroup>
    </form>
  );
}

function PlaceholderLabelForm({ placeholder }: { placeholder: Placeholder }) {
  const updatePlaceholder = useAtomSet(atoms.updatePlaceholderAtom, { mode: "promiseExit" });
  const form = useForm({
    defaultValues: {
      label: placeholder.label,
    },
    onSubmit: async ({ value }) => {
      if (value.label === placeholder.label) {
        return;
      }

      const decoded = Schema.decodeUnknownExit(PlaceholderUpdateInput)({
        id: placeholder.id,
        key: placeholder.key,
        label: value.label,
      });

      if (Exit.isFailure(decoded)) {
        return;
      }

      const exit = await updatePlaceholder(decoded.value);

      if (Exit.isFailure(exit)) {
        toast.error(
          Option.match(Cause.findErrorOption(exit.cause), {
            onNone: () => "Something went wrong while saving",
            onSome: (error) =>
              Match.valueTags(error, {
                EntityNotFound: ({ entity }) => `${entity} not found`,
                EntityConflict: ({ field }) => `${field} is already in use`,
                StorageError: ({ message }) => message,
              }),
          }),
        );
        return;
      }

      form.reset({ label: decoded.value.label });
    },
  });

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <form.Field
        name="label"
        validators={{
          onBlur: schemaError(NonEmptyTrimmedString, "Required"),
          onSubmit: schemaError(NonEmptyTrimmedString, "Required"),
        }}
        children={(field) => {
          const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;

          return (
            <Field data-invalid={isInvalid} dir="rtl">
              <FieldLabel htmlFor={`placeholder-${placeholder.id}`}>{placeholder.key}</FieldLabel>
              <Input
                aria-invalid={isInvalid}
                dir="rtl"
                id={`placeholder-${placeholder.id}`}
                lang="ur"
                name={field.name}
                onBlur={() => {
                  field.handleBlur();
                  void form.handleSubmit();
                }}
                onChange={(event) => field.handleChange(event.target.value)}
                value={field.state.value}
              />
              <FieldDescription dir="ltr">@{placeholder.key}@</FieldDescription>
              {isInvalid ? <FieldError errors={field.state.meta.errors} /> : null}
            </Field>
          );
        }}
      />
    </form>
  );
}

function PlaceholdersUnavailable() {
  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-6 p-4">
      <section className="flex flex-col gap-1">
        <h1 className="text-xl font-medium">Placeholders</h1>
      </section>
      <AddPlaceholderForm />
      <p className="text-sm text-muted-foreground">Could not load placeholders.</p>
    </main>
  );
}

function PlaceholderPage({ placeholders }: { placeholders: readonly Placeholder[] }) {
  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-6 p-4">
      <section className="flex flex-col gap-1">
        <h1 className="text-xl font-medium">Placeholders</h1>
      </section>
      <AddPlaceholderForm />
      <FieldGroup className="grid gap-3 md:grid-cols-2" dir="rtl">
        {placeholders.map((placeholder) => (
          <PlaceholderLabelForm key={placeholder.id} placeholder={placeholder} />
        ))}
      </FieldGroup>
    </main>
  );
}

function PlaceholdersSkeleton() {
  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-6 p-4">
      <Skeleton className="h-7 w-36" />
      <Skeleton className="h-16 w-full" />
      <div className="grid gap-3 md:grid-cols-2">
        {Array.from({ length: 8 }, (_, index) => (
          <Skeleton key={index} className="h-16" />
        ))}
      </div>
    </main>
  );
}

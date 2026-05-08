import { useEffect, useId } from "react";
import { useForm } from "@tanstack/react-form";
import { Button } from "#/components/ui/button";
import { Field, FieldContent, FieldError, FieldGroup, FieldLabel } from "#/components/ui/field";
import { FirDatePickerInput } from "#/components/fir-date-picker-input";
import { Input } from "#/components/ui/input";
import { Textarea } from "#/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import {
  createEmptyFirRecord,
  FIR_STATUS_OPTIONS,
  firSchema,
  getFirStatusColor,
  getFirStatusLabel,
} from "#/lib/fir";
import type { FirRecord } from "#/lib/fir";
import { firCollection, getNextFirId } from "#/db-collections";
import { useLiveQuery } from "@tanstack/react-db";
import { cn } from "#/lib/utils";

const formSchema = firSchema.omit({ id: true });
type FirFormValues = Omit<FirRecord, "id">;

type CreateFirFormProps = {
  className?: string;
  fir?: FirRecord;
  onSuccess?: (firId: number) => void;
};

function getFirFormValues(fir?: FirRecord): FirFormValues {
  if (!fir) {
    return createEmptyFirRecord();
  }

  const { id: _id, ...values } = fir;
  return values;
}

export function CreateFirForm({ className, fir, onSuccess }: CreateFirFormProps) {
  const formId = useId();
  const { data: records } = useLiveQuery(firCollection);
  const isEditing = Boolean(fir);

  const form = useForm({
    defaultValues: getFirFormValues(fir),
    validators: {
      onSubmit: formSchema,
    },
    onSubmit: async ({ value }) => {
      if (fir) {
        firCollection.update(fir.id, (draft) => {
          Object.assign(draft, value);
        });
        onSuccess?.(fir.id);
        return;
      }

      const firId = getNextFirId(records);

      firCollection.insert({
        ...value,
        id: firId,
      });

      form.reset();
      onSuccess?.(firId);
    },
  });

  useEffect(() => {
    form.reset(getFirFormValues(fir));
  }, [fir, form]);

  return (
    <form
      id={formId}
      className={cn("grid w-full max-w-5xl gap-4 py-4", className)}
      dir="rtl"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
      onKeyDown={(event) => {
        if (event.key.toLowerCase() !== "s" || (!event.ctrlKey && !event.metaKey)) {
          return;
        }

        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      <h1 className="text-xl font-semibold">
        {isEditing ? "ترمیم ایف آئی آر" : "اندراج ایف آئی آر"}
      </h1>
      <FieldGroup className="grid gap-2 lg:grid-cols-2">
        <form.Field
          name="fir_no"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-fir-no`}>ایف آئی آر نمبر</FieldLabel>
                <Input
                  id={`${formId}-fir-no`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  aria-invalid={isInvalid}
                  placeholder="23/26"
                />
                <FieldError errors={field.state.meta.errors} />
              </Field>
            );
          }}
        />
        <form.Field
          name="status"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldContent>
                  <FieldLabel htmlFor={`${formId}-status`}>حالت</FieldLabel>
                  <FieldError errors={field.state.meta.errors} />
                </FieldContent>
                <Select
                  name={field.name}
                  value={field.state.value}
                  onValueChange={(value) => {
                    if (value) {
                      field.handleChange(value);
                    }
                  }}
                >
                  <SelectTrigger
                    id={`${formId}-status`}
                    aria-invalid={isInvalid}
                    className="w-full min-w-55"
                  >
                    <SelectValue placeholder="حالت منتخب کریں">
                      <span className="flex items-center gap-2">
                        <span
                          aria-hidden="true"
                          className={cn(
                            "size-1.5 rounded-full",
                            getFirStatusColor(field.state.value),
                          )}
                        />
                        {getFirStatusLabel(field.state.value)}
                      </span>
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent align="center" alignItemWithTrigger={false} dir="rtl">
                    {FIR_STATUS_OPTIONS.map((status) => (
                      <SelectItem key={status} value={status}>
                        <span
                          aria-hidden="true"
                          className={cn("size-1.5 rounded-full my-auto", getFirStatusColor(status))}
                        />
                        {getFirStatusLabel(status)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            );
          }}
        />
        <form.Field
          name="date"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-date`}>ایف آئی آر کی تاریخ</FieldLabel>
                <FirDatePickerInput
                  id={`${formId}-date`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={field.handleChange}
                  ariaInvalid={isInvalid}
                  placeholder="June 01, 2025"
                />
                <FieldError errors={field.state.meta.errors} />
              </Field>
            );
          }}
        />
        <form.Field
          name="incident_date"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-incident-date`}>تاریخ وقوعہ</FieldLabel>
                <FirDatePickerInput
                  id={`${formId}-incident-date`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={field.handleChange}
                  ariaInvalid={isInvalid}
                  placeholder="June 01, 2025"
                />
                <FieldError errors={field.state.meta.errors} />
              </Field>
            );
          }}
        />
        <form.Field
          name="offence"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-offence`}>جرم</FieldLabel>
                <Input
                  id={`${formId}-offence`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  aria-invalid={isInvalid}
                  placeholder="411/379"
                />
                <FieldError errors={field.state.meta.errors} />
              </Field>
            );
          }}
        />
        <form.Field
          name="mobile"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-mobile`}>موبائل نمبر</FieldLabel>
                <Input
                  id={`${formId}-mobile`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  aria-invalid={isInvalid}
                  placeholder="03001234567"
                />
                <FieldError errors={field.state.meta.errors} />
              </Field>
            );
          }}
        />
        <form.Field
          name="NIC"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-nic`}>شناختی کارڈ نمبر</FieldLabel>
                <Input
                  id={`${formId}-nic`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  aria-invalid={isInvalid}
                  placeholder="3520288701547"
                />
                <FieldError errors={field.state.meta.errors} />
              </Field>
            );
          }}
        />
      </FieldGroup>

      <form.Field
        name="accused"
        children={(field) => {
          const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
          return (
            <Field data-invalid={isInvalid}>
              <FieldLabel htmlFor={`${formId}-accused`}>نام ملزم و سکونت</FieldLabel>
              <Textarea
                id={`${formId}-accused`}
                name={field.name}
                value={field.state.value}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                aria-invalid={isInvalid}
                placeholder="نام ملزم و سکونت"
                dir="rtl"
                className="min-h-24 resize-y"
              />
              <FieldError errors={field.state.meta.errors} />
            </Field>
          );
        }}
      />

      <form.Field
        name="witness"
        children={(field) => {
          const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
          return (
            <Field data-invalid={isInvalid}>
              <FieldLabel htmlFor={`${formId}-witness`}>گواہان</FieldLabel>
              <Textarea
                id={`${formId}-witness`}
                name={field.name}
                value={field.state.value}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                aria-invalid={isInvalid}
                placeholder="گواہان 1 / گواہان 2"
                dir="rtl"
                className="min-h-20 resize-y"
              />
              <FieldError errors={field.state.meta.errors} />
            </Field>
          );
        }}
      />
      <div className="flex items-center justify-end gap-2" dir="ltr">
        <Button type="button" variant="outline" onClick={() => form.reset(getFirFormValues(fir))}>
          Reset
        </Button>
        <Button type="submit" form={formId}>
          {isEditing ? "Update" : "Save"}
        </Button>
      </div>
    </form>
  );
}

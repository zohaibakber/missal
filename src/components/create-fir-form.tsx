import { useId } from "react";
import { useForm } from "@tanstack/react-form";
import { Button } from "#/components/ui/button";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "#/components/ui/field";
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
import { createEmptyFirRecord, firSchema, FIR_STATUS_OPTIONS } from "#/lib/fir";
import { firCollection, getNextFirId } from "#/db-collections";
import { useLiveQuery } from "@tanstack/react-db";

const formSchema = firSchema.omit({ id: true });

type CreateFirFormProps = {
  onSuccess?: () => void;
};

export function CreateFirForm({ onSuccess }: CreateFirFormProps) {
  const formId = useId();
  const { data: records } = useLiveQuery(firCollection);

  const form = useForm({
    defaultValues: createEmptyFirRecord(),
    validators: {
      onSubmit: formSchema,
    },
    onSubmit: async ({ value }) => {
      firCollection.insert({
        ...value,
        id: getNextFirId(records),
      });

      form.reset();
      onSuccess?.();
    },
  });

  return (
    <>
      <form
        id={formId}
        className="grid gap-2.5 p-4 py-0"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <FieldGroup className="grid gap-2 lg:grid-cols-2">
          <form.Field
            name="fir_no"
            children={(field) => {
              const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
              return (
                <Field data-invalid={isInvalid}>
                  <FieldLabel htmlFor={`${formId}-fir-no`}>FIR No</FieldLabel>
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
                    <FieldLabel htmlFor={`${formId}-status`}>Status</FieldLabel>
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
                      <SelectValue placeholder="Select status" />
                    </SelectTrigger>
                    <SelectContent align="end">
                      {FIR_STATUS_OPTIONS.map((status) => (
                        <SelectItem key={status} value={status}>
                          {status}
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
                  <FieldLabel htmlFor={`${formId}-date`}>FIR Date</FieldLabel>
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
                  <FieldLabel htmlFor={`${formId}-incident-date`}>Incident Date</FieldLabel>
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
                  <FieldLabel htmlFor={`${formId}-offence`}>Offence</FieldLabel>
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
                  <FieldLabel htmlFor={`${formId}-mobile`}>Mobile</FieldLabel>
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
                  <FieldLabel htmlFor={`${formId}-nic`}>CNIC</FieldLabel>
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
                <FieldLabel htmlFor={`${formId}-accused`}>Accused</FieldLabel>
                <Textarea
                  id={`${formId}-accused`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  aria-invalid={isInvalid}
                  placeholder="نام ملزم و سکونت"
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
                <FieldLabel htmlFor={`${formId}-witness`}>Witness</FieldLabel>
                <Textarea
                  id={`${formId}-witness`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  aria-invalid={isInvalid}
                  placeholder="گواہان 1 / گواہان 2"
                  className="min-h-20 resize-y"
                />
                <FieldDescription>
                  Optional, but recommended for better dataset searchability.
                </FieldDescription>
                <FieldError errors={field.state.meta.errors} />
              </Field>
            );
          }}
        />
      </form>

      <div className="flex items-center justify-end gap-2 border-t bg-muted/40 px-6 py-4">
        <Button type="button" variant="outline" onClick={() => form.reset()}>
          Reset
        </Button>
        <Button type="submit" form={formId}>
          Save FIR
        </Button>
      </div>
    </>
  );
}

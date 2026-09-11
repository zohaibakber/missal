import { useEffect, useId } from "react";
import { useAtomSet } from "@effect/atom-react";
import { useForm } from "@tanstack/react-form";
import { Cause, Exit, Match, Option, Schema } from "effect";
import { toast } from "sonner";
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
  FirCreateInput,
  FirUpdateInput,
  getFirStatusColor,
  getFirStatusLabel,
  type FirFormValues,
  type FirRecord,
} from "#/lib/fir";
import { formatDate } from "#/lib/date";
import { atoms } from "#/state/atoms";
import { cn } from "#/lib/utils";

type FirFormSharedProps = {
  className?: string;
  onSuccess?: (firId: number) => void;
};

type FirFormProps = FirFormSharedProps &
  (
    | { kind: "create" }
    | {
        kind: "edit";
        fir: FirRecord;
      }
  );

function getFirFormValues(fir?: FirRecord): FirFormValues {
  if (!fir) {
    return createEmptyFirRecord();
  }

  const { id: _id, ...values } = fir;
  return {
    ...createEmptyFirRecord(),
    ...values,
    arrest_date: formatDate(values.arrest_date ?? ""),
    date: formatDate(values.date ?? ""),
    incident_date: formatDate(values.incident_date ?? ""),
    investigation_officer: values.investigation_officer ?? "",
  };
}

function FirForm(props: FirFormProps) {
  const { className, onSuccess } = props;
  const fir = props.kind === "edit" ? props.fir : undefined;
  const formId = useId();
  const isEditing = Boolean(fir);
  const createFir = useAtomSet(atoms.createFirAtom, { mode: "promiseExit" });
  const updateFir = useAtomSet(atoms.updateFirAtom, { mode: "promiseExit" });

  const form = useForm({
    defaultValues: getFirFormValues(fir),
    validators: {
      onSubmit: ({ value }) => {
        const required = ["fir_no", "date", "offence", "accused", "incident_date"] as const;
        const fields: Partial<Record<(typeof required)[number], string>> = {};

        for (const key of required) {
          if (!value[key].trim()) {
            fields[key] = "Required";
          }
        }

        return Object.keys(fields).length ? { fields } : undefined;
      },
    },
    onSubmit: async ({ value }) => {
      if (fir) {
        const decoded = Schema.decodeUnknownExit(FirUpdateInput)({
          ...value,
          id: fir.id,
          arrest_date: formatDate(value.arrest_date),
          date: formatDate(value.date),
          incident_date: formatDate(value.incident_date),
        });

        if (Exit.isFailure(decoded)) {
          toast.error("Please fill in the required fields");
          return;
        }

        const exit = await updateFir(decoded.value);

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

        onSuccess?.(exit.value.id);
        return;
      }

      const decoded = Schema.decodeUnknownExit(FirCreateInput)({
        ...value,
        arrest_date: formatDate(value.arrest_date),
        date: formatDate(value.date),
        incident_date: formatDate(value.incident_date),
      });

      if (Exit.isFailure(decoded)) {
        toast.error("Please fill in the required fields");
        return;
      }

      const exit = await createFir(decoded.value);

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
      onSuccess?.(exit.value.id);
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
                  placeholder="13-01-2026"
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
                  placeholder="13-01-2026"
                />
                <FieldError errors={field.state.meta.errors} />
              </Field>
            );
          }}
        />
        <form.Field
          name="arrest_date"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-arrest-date`}>تاریخ گرفتاری</FieldLabel>
                <FirDatePickerInput
                  id={`${formId}-arrest-date`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={field.handleChange}
                  ariaInvalid={isInvalid}
                  placeholder="13-01-2026"
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
          name="investigation_officer"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-investigation-officer`}>تفتیشی افسر</FieldLabel>
                <Input
                  id={`${formId}-investigation-officer`}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  aria-invalid={isInvalid}
                  placeholder="نام تفتیشی افسر"
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

export function CreateFirForm(props: FirFormSharedProps) {
  return <FirForm {...props} kind="create" />;
}

export function EditFirForm({ fir, ...props }: FirFormSharedProps & { fir: FirRecord }) {
  return <FirForm {...props} fir={fir} kind="edit" />;
}

import { useEffect, useId } from "react";
import { useAtomSet } from "@effect/atom-react";
import { useForm } from "@tanstack/react-form";
import { Exit, Schema } from "effect";
import { toast } from "#/components/ui/toast";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { Button } from "#/components/ui/button";
import {
  Field,
  FieldContent,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldSet,
  FieldLegend,
} from "#/components/ui/field";
import { FirDatePickerInput } from "#/components/fir-date-picker-input";
import { Input } from "#/components/ui/input";
import { Add01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectGroup,
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
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
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
    accused: [...values.accused],
    witness: [...values.witness],
    zimni: [...values.zimni],
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
        const required = ["fir_no", "date", "offence", "incident_date"] as const;
        const fields: Partial<Record<(typeof required)[number], string>> = {};

        for (const key of required) {
          if (!value[key].trim()) {
            fields[key] = "یہ خانہ پُر کریں";
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
          toast.add({ title: "Please fill in the required fields", type: "error" });
          return;
        }

        const exit = await updateFir(decoded.value);

        if (Exit.isFailure(exit)) {
          toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
          return;
        }

        form.reset(getFirFormValues(exit.value));
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
        toast.add({ title: "Please fill in the required fields", type: "error" });
        return;
      }

      const exit = await createFir(decoded.value);

      if (Exit.isFailure(exit)) {
        toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
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
      className={cn("missal-fir-form flex w-full max-w-3xl flex-col gap-6 py-2", className)}
      dir="rtl"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
      onKeyDown={(event) => {
        if (event.key.toLowerCase() !== "s" || (!event.ctrlKey && !event.metaKey)) {
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <UnsavedChanges isDirty={() => form.state.isDirty} />
      <FieldGroup className="grid gap-x-6 gap-y-5 sm:grid-cols-2">
        <form.Field
          name="fir_no"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-fir-no`}>ایف آئی آر نمبر</FieldLabel>
                <Input
                  dir="ltr"
                  className="text-right"
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
                  <SelectContent align="start" alignItemWithTrigger={false} dir="rtl">
                    <SelectGroup>
                      {FIR_STATUS_OPTIONS.map((status) => (
                        <SelectItem key={status} value={status}>
                          <span
                            aria-hidden="true"
                            className={cn(
                              "size-1.5 rounded-full my-auto",
                              getFirStatusColor(status),
                            )}
                          />
                          {getFirStatusLabel(status)}
                        </SelectItem>
                      ))}
                    </SelectGroup>
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
          name="NIC"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={`${formId}-nic`}>شناختی کارڈ نمبر</FieldLabel>
                <Input
                  dir="ltr"
                  className="text-right"
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

      {(
        [
          {
            name: "accused",
            label: "ملزمان",
            entry: "ملزم",
            placeholder: "نام، ولدیت اور سکونت",
            required: true,
          },
          {
            name: "witness",
            label: "گواہان",
            entry: "گواہ",
            placeholder: "نام اور تفصیل",
            required: false,
          },
          {
            name: "zimni",
            label: "ضمنی",
            entry: "ضمنی",
            placeholder: "ضمنی کی تفصیل",
            required: false,
          },
        ] as const
      ).map((section) => (
        <form.Field
          key={section.name}
          name={section.name}
          mode="array"
          validators={{
            onSubmit: ({ value }) =>
              (section.required && value.length === 0) || value.some((text) => !text.trim())
                ? "ہر اندراج مکمل کریں یا خالی اندراج ہٹا دیں"
                : undefined,
          }}
        >
          {(arrayField) => (
            <FieldSet className="gap-3">
              <FieldLegend variant="label">{section.label}</FieldLegend>
              <FieldGroup className="gap-3">
                {arrayField.state.value.map((_, index) => (
                  <form.Field key={index} name={`${section.name}[${index}]`}>
                    {(field) => {
                      const invalid =
                        !field.state.value.trim() && arrayField.state.meta.errors.length > 0;
                      return (
                        <Field data-invalid={invalid}>
                          <FieldLabel
                            className="sr-only"
                            htmlFor={`${formId}-${section.name}-${index}`}
                          >
                            {section.entry} {index + 1}
                          </FieldLabel>
                          <InputGroup>
                            <InputGroupInput
                              id={`${formId}-${section.name}-${index}`}
                              name={field.name}
                              aria-invalid={invalid}
                              value={field.state.value}
                              onBlur={field.handleBlur}
                              onChange={(event) => field.handleChange(event.target.value)}
                              placeholder={`${section.entry} ${index + 1} — ${section.placeholder}`}
                            />
                            <InputGroupAddon align="inline-end">
                              <InputGroupButton
                                size="icon-xs"
                                variant="ghost"
                                type="button"
                                disabled={section.required && arrayField.state.value.length === 1}
                                aria-label={`${section.entry} ${index + 1} ہٹائیں`}
                                onClick={() => arrayField.removeValue(index)}
                              >
                                <HugeiconsIcon icon={Cancel01Icon} />
                              </InputGroupButton>
                            </InputGroupAddon>
                          </InputGroup>
                        </Field>
                      );
                    }}
                  </form.Field>
                ))}
              </FieldGroup>
              <FieldError errors={arrayField.state.meta.errors} />
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="self-start"
                onClick={() => {
                  const index = arrayField.state.value.length;
                  arrayField.pushValue("");
                  requestAnimationFrame(() =>
                    document.getElementById(`${formId}-${section.name}-${index}`)?.focus(),
                  );
                }}
              >
                <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
                {section.entry} شامل کریں
              </Button>
            </FieldSet>
          )}
        </form.Field>
      ))}
      <form.Subscribe selector={(state) => state.isSubmitting}>
        {(isSubmitting) => (
          <div
            dir="ltr"
            className="sticky bottom-0 flex justify-end items-center gap-2 border-t bg-background py-3"
          >
            <Button type="submit" form={formId} disabled={isSubmitting}>
              {isSubmitting ? "Saving…" : isEditing ? "Save changes" : "Create FIR"}
            </Button>
            <Button
              type="button"
              variant="ghost"
              disabled={isSubmitting}
              onClick={() => form.reset(getFirFormValues(fir))}
            >
              Reset changes
            </Button>
          </div>
        )}
      </form.Subscribe>
    </form>
  );
}

export function CreateFirForm(props: FirFormSharedProps) {
  return <FirForm {...props} kind="create" />;
}

export function EditFirForm({ fir, ...props }: FirFormSharedProps & { fir: FirRecord }) {
  return <FirForm {...props} fir={fir} kind="edit" />;
}

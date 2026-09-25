import { createContext, use, useEffect, useId, useRef, type ReactNode } from "react";
import { useAtomSet } from "@effect/atom-react";
import { useForm } from "@tanstack/react-form";
import { Exit, Schema } from "effect";
import { Add01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { FirDatePickerInput } from "#/components/fir-date-picker-input";
import { FirStatusDot } from "#/components/fir-status-badge";
import { Hint } from "#/components/hint";
import { SaveStatus } from "#/components/pane";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { Button } from "#/components/ui/button";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSeparator,
  FieldSet,
} from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import { Spinner } from "#/components/ui/spinner";
import { toast } from "#/components/ui/toast";
import { useShortcut } from "#/hooks/use-shortcut";
import { formatDate } from "#/lib/date";
import {
  createEmptyFirRecord,
  FIR_STATUS_OPTIONS,
  FirCreateInput,
  FirUpdateInput,
  getFirStatusLabel,
  type FirFormValues,
  type FirRecord,
} from "#/lib/fir";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { cn } from "#/lib/utils";
import { atoms } from "#/state/atoms";

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

function useFirFormState(fir: FirRecord | undefined, onSuccess?: (firId: number) => void) {
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
            fields[key] = "Required";
          }
        }

        return Object.keys(fields).length ? { fields } : undefined;
      },
    },
    onSubmit: async ({ value }) => {
      const dates = {
        arrest_date: formatDate(value.arrest_date),
        date: formatDate(value.date),
        incident_date: formatDate(value.incident_date),
      };

      if (fir) {
        const decoded = Schema.decodeUnknownExit(FirUpdateInput)({
          ...value,
          ...dates,
          id: fir.id,
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

      const decoded = Schema.decodeUnknownExit(FirCreateInput)({ ...value, ...dates });
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

  return form;
}

type FirFormContextValue = {
  form: ReturnType<typeof useFirFormState>;
  formId: string;
  fir: FirRecord | undefined;
};

const FirFormContext = createContext<FirFormContextValue | null>(null);

function useFirForm() {
  const context = use(FirFormContext);
  if (!context) throw new Error("FirForm parts must be rendered inside <FirForm>.");
  return context;
}

type FirFormProps = {
  fir?: FirRecord;
  onSuccess?: (firId: number) => void;
  className?: string;
  children: ReactNode;
};

function FirForm({ fir, onSuccess, className, children }: FirFormProps) {
  const formId = useId();
  const formRef = useRef<HTMLFormElement>(null);
  const form = useFirFormState(fir, onSuccess);

  useShortcut("save", () => void form.handleSubmit(), { target: formRef, allowInOverlay: true });

  return (
    <FirFormContext value={{ form, formId, fir }}>
      <form
        ref={formRef}
        id={formId}
        className={className}
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <UnsavedChanges isDirty={() => form.state.isDirty} />
        {children}
      </form>
    </FirFormContext>
  );
}

function FirFormStatus() {
  const { form } = useFirForm();
  return (
    <form.Subscribe selector={(state) => state.isDirty}>
      {(isDirty) => (isDirty ? <SaveStatus dirty /> : null)}
    </form.Subscribe>
  );
}

function FirFormReset() {
  const { form, fir } = useFirForm();
  return (
    <form.Subscribe
      selector={(state) => ({ isDirty: state.isDirty, isSubmitting: state.isSubmitting })}
    >
      {({ isDirty, isSubmitting }) => (
        <Button
          type="button"
          variant="subtle"
          size="sm"
          disabled={isSubmitting || !isDirty}
          onClick={() => form.reset(getFirFormValues(fir))}
        >
          Reset
        </Button>
      )}
    </form.Subscribe>
  );
}

function FirFormSubmit({ children }: { children: ReactNode }) {
  const { form, formId } = useFirForm();
  return (
    <form.Subscribe selector={(state) => state.isSubmitting}>
      {(isSubmitting) => (
        <Hint label="Save" shortcut="save">
          <Button type="submit" form={formId} size="sm" disabled={isSubmitting}>
            {isSubmitting ? <Spinner data-icon="inline-start" /> : null}
            {children}
          </Button>
        </Hint>
      )}
    </form.Subscribe>
  );
}

const repeatableSections = [
  {
    name: "accused",
    label: "ملزمان",
    entry: "ملزم",
    add: "Add accused",
    placeholder: "نام، ولدیت اور سکونت",
    required: true,
  },
  {
    name: "witness",
    label: "گواہان",
    entry: "گواہ",
    add: "Add witness",
    placeholder: "نام اور تفصیل",
    required: false,
  },
  {
    name: "zimni",
    label: "ضمنی",
    entry: "ضمنی",
    add: "Add zimni",
    placeholder: "ضمنی کی تفصیل",
    required: false,
  },
] as const;

function FirFormFields({ className }: { className?: string }) {
  const { form, formId, fir } = useFirForm();
  const id = (name: string) => `${formId}-${name}`;

  return (
    <div dir="rtl" lang="ur" className={cn("missal-fir-form flex flex-col gap-8", className)}>
      <FieldGroup className="grid gap-x-5 gap-y-5 sm:grid-cols-2">
        <form.Field
          name="fir_no"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={id("fir-no")}>ایف آئی آر نمبر</FieldLabel>
                <Input
                  autoFocus={!fir}
                  dir="ltr"
                  className="text-end"
                  id={id("fir-no")}
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
          children={(field) => (
            <Field>
              <FieldLabel htmlFor={id("status")}>حالت</FieldLabel>
              <Select
                name={field.name}
                value={field.state.value}
                onValueChange={(value) => {
                  if (value) field.handleChange(value);
                }}
              >
                <SelectTrigger id={id("status")} className="w-full">
                  <SelectValue>
                    <span className="flex items-center gap-2 text-ur">
                      <FirStatusDot status={field.state.value} />
                      {getFirStatusLabel(field.state.value)}
                    </span>
                  </SelectValue>
                </SelectTrigger>
                <SelectContent align="start" alignItemWithTrigger={false} dir="rtl">
                  <SelectGroup>
                    {FIR_STATUS_OPTIONS.map((status) => (
                      <SelectItem key={status} value={status}>
                        <span className="flex items-center gap-2 text-ur">
                          <FirStatusDot status={status} />
                          {getFirStatusLabel(status)}
                        </span>
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          )}
        />
        {(
          [
            { name: "date", label: "ایف آئی آر کی تاریخ" },
            { name: "incident_date", label: "تاریخ وقوعہ" },
            { name: "arrest_date", label: "تاریخ گرفتاری" },
          ] as const
        ).map((date) => (
          <form.Field
            key={date.name}
            name={date.name}
            children={(field) => {
              const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
              return (
                <Field data-invalid={isInvalid}>
                  <FieldLabel htmlFor={id(date.name)}>{date.label}</FieldLabel>
                  <FirDatePickerInput
                    id={id(date.name)}
                    name={field.name}
                    value={field.state.value}
                    onBlur={field.handleBlur}
                    onChange={field.handleChange}
                    ariaInvalid={isInvalid}
                    placeholder="DD-MM-YYYY"
                  />
                  <FieldError errors={field.state.meta.errors} />
                </Field>
              );
            }}
          />
        ))}
        <form.Field
          name="offence"
          children={(field) => {
            const isInvalid = field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={id("offence")}>جرم</FieldLabel>
                <Input
                  id={id("offence")}
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
          children={(field) => (
            <Field>
              <FieldLabel htmlFor={id("nic")}>شناختی کارڈ نمبر</FieldLabel>
              <Input
                dir="ltr"
                className="text-end"
                inputMode="numeric"
                id={id("nic")}
                name={field.name}
                value={field.state.value}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                placeholder="3520288701547"
              />
            </Field>
          )}
        />
      </FieldGroup>

      {repeatableSections.map((section) => (
        <form.Field
          key={section.name}
          name={section.name}
          mode="array"
          validators={{
            onSubmit: ({ value }) =>
              (section.required && value.length === 0) || value.some((text) => !text.trim())
                ? "Fill in or remove empty entries"
                : undefined,
          }}
        >
          {(arrayField) => (
            <>
              <FieldSeparator />
              <FieldSet>
                <FieldLegend variant="label">{section.label}</FieldLegend>
                <FieldGroup className="gap-2">
                  {arrayField.state.value.map((_, index) => (
                    <form.Field key={index} name={`${section.name}[${index}]`}>
                      {(field) => {
                        const invalid =
                          !field.state.value.trim() && arrayField.state.meta.errors.length > 0;
                        return (
                          <Field data-invalid={invalid}>
                            <FieldLabel
                              className="sr-only"
                              htmlFor={id(`${section.name}-${index}`)}
                            >
                              {section.entry} {index + 1}
                            </FieldLabel>
                            <InputGroup>
                              <InputGroupAddon align="inline-start">
                                <span className="w-4 text-center text-xs text-muted-foreground tabular-nums">
                                  {index + 1}
                                </span>
                              </InputGroupAddon>
                              <InputGroupInput
                                id={id(`${section.name}-${index}`)}
                                name={field.name}
                                aria-invalid={invalid}
                                value={field.state.value}
                                onBlur={field.handleBlur}
                                onChange={(event) => field.handleChange(event.target.value)}
                                placeholder={section.placeholder}
                              />
                              {section.required && arrayField.state.value.length === 1 ? null : (
                                <InputGroupAddon align="inline-end">
                                  <InputGroupButton
                                    size="icon-xs"
                                    variant="ghost"
                                    type="button"
                                    aria-label={`Remove ${section.entry} ${index + 1}`}
                                    onClick={() => arrayField.removeValue(index)}
                                  >
                                    <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
                                  </InputGroupButton>
                                </InputGroupAddon>
                              )}
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
                  variant="subtle"
                  size="sm"
                  className="-me-2.5 self-start"
                  dir="ltr"
                  lang="en"
                  onClick={() => {
                    const index = arrayField.state.value.length;
                    arrayField.pushValue("");
                    requestAnimationFrame(() =>
                      document.getElementById(id(`${section.name}-${index}`))?.focus(),
                    );
                  }}
                >
                  <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
                  {section.add}
                </Button>
              </FieldSet>
            </>
          )}
        </form.Field>
      ))}
    </div>
  );
}

export { FirForm, FirFormFields, FirFormReset, FirFormStatus, FirFormSubmit };

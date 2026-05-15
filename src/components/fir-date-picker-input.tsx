import * as React from "react";
import { Calendar03Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

import { Calendar } from "#/components/ui/calendar";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import { Popover, PopoverContent, PopoverTrigger } from "#/components/ui/popover";

function formatDate(date: Date | undefined) {
  if (!date) {
    return "";
  }

  return date.toLocaleDateString("en-US", {
    day: "2-digit",
    month: "long",
    year: "numeric",
  });
}

function parseDate(value: string | undefined) {
  if (!value?.trim()) {
    return undefined;
  }

  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? undefined : parsed;
}

type FirDatePickerInputProps = {
  ariaInvalid?: boolean;
  dir?: React.HTMLAttributes<HTMLDivElement>["dir"];
  id: string;
  name: string;
  onBlur?: React.FocusEventHandler<HTMLInputElement>;
  onChange: (value: string) => void;
  placeholder: string;
  value?: string;
};

export function FirDatePickerInput({
  ariaInvalid,
  dir,
  id,
  name,
  onBlur,
  onChange,
  placeholder,
  value = "",
}: FirDatePickerInputProps) {
  const [open, setOpen] = React.useState(false);
  const parsedValue = React.useMemo(() => parseDate(value), [value]);
  const [month, setMonth] = React.useState<Date | undefined>(parsedValue);

  React.useEffect(() => {
    if (parsedValue) {
      setMonth(parsedValue);
    }
  }, [parsedValue]);

  return (
    <InputGroup dir={dir}>
      <InputGroupInput
        id={id}
        name={name}
        value={value}
        placeholder={placeholder}
        aria-invalid={ariaInvalid}
        onBlur={onBlur}
        onChange={(event) => {
          const nextValue = event.target.value;
          onChange(nextValue);

          const nextDate = parseDate(nextValue);
          if (nextDate) {
            setMonth(nextDate);
          }
        }}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            event.preventDefault();
            setOpen(true);
          }
        }}
      />
      <InputGroupAddon align="inline-end">
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger
            render={
              <InputGroupButton
                id={`${id}-picker`}
                variant="ghost"
                size="icon-xs"
                aria-label="Select date"
              />
            }
          >
            <HugeiconsIcon icon={Calendar03Icon} strokeWidth={2} />
            <span className="sr-only">Select date</span>
          </PopoverTrigger>
          <PopoverContent className="w-auto overflow-hidden p-0" align="start" sideOffset={10}>
            <Calendar
              mode="single"
              selected={parsedValue}
              month={month}
              onMonthChange={setMonth}
              captionLayout="dropdown"
              onSelect={(selectedDate) => {
                onChange(formatDate(selectedDate));
                setOpen(false);
              }}
            />
          </PopoverContent>
        </Popover>
      </InputGroupAddon>
    </InputGroup>
  );
}

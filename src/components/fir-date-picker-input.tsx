import { useState, type FocusEventHandler, type HTMLAttributes } from "react";
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
import { formatDate, parseDate } from "#/lib/date";

type FirDatePickerInputProps = {
  ariaInvalid?: boolean;
  dir?: HTMLAttributes<HTMLDivElement>["dir"];
  id: string;
  name: string;
  onBlur?: FocusEventHandler<HTMLInputElement>;
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
  const [open, setOpen] = useState(false);
  const [calendarMonth, setCalendarMonth] = useState<Date | undefined>(undefined);
  const parsedValue = parseDate(value);
  const month = calendarMonth ?? parsedValue;

  return (
    <InputGroup dir={dir}>
      <InputGroupInput
        id={id}
        name={name}
        value={value}
        placeholder={placeholder}
        aria-invalid={ariaInvalid}
        inputMode="numeric"
        dir="ltr"
        className="text-right"
        onBlur={(event) => {
          if (parsedValue) {
            onChange(formatDate(parsedValue));
          }

          onBlur?.(event);
        }}
        onChange={(event) => {
          const nextValue = event.target.value;
          onChange(nextValue);

          const nextDate = parseDate(nextValue);
          if (nextDate) {
            setCalendarMonth(nextDate);
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
        <Popover
          open={open}
          onOpenChange={(nextOpen) => {
            setOpen(nextOpen);

            if (!nextOpen) {
              setCalendarMonth(undefined);
            }
          }}
        >
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
              onMonthChange={setCalendarMonth}
              captionLayout="dropdown"
              onSelect={(selectedDate) => {
                onChange(selectedDate ? formatDate(selectedDate) : "");
                setOpen(false);
                setCalendarMonth(undefined);
              }}
            />
          </PopoverContent>
        </Popover>
      </InputGroupAddon>
    </InputGroup>
  );
}

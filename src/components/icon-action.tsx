import type { ComponentProps } from "react";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Button } from "#/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "#/components/ui/tooltip";
import type { ShortcutId } from "#/lib/shortcuts";

type IconActionProps = Omit<ComponentProps<typeof Button>, "size" | "aria-label"> & {
  label: string;
  shortcut?: ShortcutId;
};

export function IconAction({ label, shortcut, variant = "ghost", ...props }: IconActionProps) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button type="button" variant={variant} size="icon-sm" aria-label={label} {...props} />
        }
      />
      <TooltipContent>
        {label}
        {shortcut ? <ShortcutKbd id={shortcut} /> : null}
      </TooltipContent>
    </Tooltip>
  );
}

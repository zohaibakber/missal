import type { ComponentProps, ReactElement } from "react";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Tooltip, TooltipContent, TooltipTrigger } from "#/components/ui/tooltip";
import type { ShortcutId } from "#/lib/shortcuts";

type HintProps = {
  /** The element that shows the hint; rendered as the tooltip trigger. */
  children: ReactElement;
  label: string;
  shortcut?: ShortcutId;
  side?: ComponentProps<typeof TooltipContent>["side"];
};

/** Tooltip naming an action and its keyboard shortcut. */
export function Hint({ children, label, shortcut, side = "bottom" }: HintProps) {
  return (
    <Tooltip>
      <TooltipTrigger render={children} />
      <TooltipContent side={side}>
        {label}
        {shortcut ? <ShortcutKbd id={shortcut} /> : null}
      </TooltipContent>
    </Tooltip>
  );
}

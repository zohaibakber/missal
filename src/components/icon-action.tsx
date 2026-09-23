import type { ComponentProps } from "react";
import { Hint } from "#/components/hint";
import { Button } from "#/components/ui/button";
import type { ShortcutId } from "#/lib/shortcuts";

type IconActionProps = Omit<ComponentProps<typeof Button>, "size" | "aria-label"> & {
  label: string;
  shortcut?: ShortcutId;
  side?: ComponentProps<typeof Hint>["side"];
};

export function IconAction({
  label,
  shortcut,
  side,
  variant = "ghost",
  ...props
}: IconActionProps) {
  return (
    <Hint label={label} shortcut={shortcut} side={side}>
      <Button type="button" variant={variant} size="icon-sm" aria-label={label} {...props} />
    </Hint>
  );
}

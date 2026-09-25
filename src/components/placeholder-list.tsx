import type { ComponentProps } from "react";
import { useAtomValue } from "@effect/atom-react";
import { Add01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Hint } from "#/components/hint";
import { SaveStatus } from "#/components/pane";
import { Button } from "#/components/ui/button";
import { Spinner } from "#/components/ui/spinner";
import { cn } from "#/lib/utils";
import { atoms } from "#/state/atoms";

function PlaceholderList({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="placeholder-list"
      dir="rtl"
      lang="ur"
      className={cn("flex flex-col gap-2", className)}
      {...props}
    />
  );
}

function PlaceholderListActions({
  addLabel,
  dirty,
  onAdd,
  onReset,
  saving,
}: {
  addLabel: string;
  dirty: boolean;
  onAdd: () => void;
  onReset: () => void;
  saving: boolean;
}) {
  return (
    <div dir="ltr" lang="en" className="flex items-center gap-2 pt-1">
      <Button type="button" size="sm" variant="subtle" disabled={saving} onClick={onAdd}>
        <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
        {addLabel}
      </Button>
      {dirty ? (
        <>
          <SaveStatus dirty className="ms-auto" />
          <Button type="button" variant="subtle" size="sm" disabled={saving} onClick={onReset}>
            Reset
          </Button>
          <Hint label="Save" shortcut="save">
            <Button type="submit" size="sm" disabled={saving}>
              {saving ? <Spinner data-icon="inline-start" /> : null}
              Save
            </Button>
          </Hint>
        </>
      ) : null}
    </div>
  );
}

function PlaceholderToken({ name }: { name: string }) {
  const { open, close } = useAtomValue(atoms.fieldMarkersAtom);
  return (
    <code dir="rtl" lang="ur" className="rounded bg-muted px-1 font-mono text-foreground">
      {open}
      {name}
      {close}
    </code>
  );
}

export { PlaceholderList, PlaceholderListActions, PlaceholderToken };

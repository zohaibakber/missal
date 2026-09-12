import { useState } from "react";
import { useAtomSet, useAtomValue, useAtomRefresh } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Cause, Exit, Option, Schema } from "effect";
import { Add01Icon, Copy01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "#/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import {
  Empty,
  EmptyHeader,
  EmptyTitle,
  EmptyDescription,
  EmptyContent,
} from "#/components/ui/empty";
import { Skeleton } from "#/components/ui/skeleton";
import { Tooltip, TooltipTrigger, TooltipContent } from "#/components/ui/tooltip";
import { UnsavedChanges } from "#/components/unsaved-changes";
import { toast } from "#/components/ui/toast";
import {
  GlobalPlaceholder,
  GlobalPlaceholderDraft,
  SaveGlobalPlaceholdersInput,
} from "#/lib/global-placeholder";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { atoms } from "#/state/atoms";

type Draft = typeof GlobalPlaceholderDraft.Type;
function draftsFrom(globals: readonly GlobalPlaceholder[]): Draft[] {
  return globals.map((item) => ({
    _tag: "Existing",
    id: item.id,
    label: item.label,
    value: item.value,
  }));
}

export function GlobalPlaceholderForm() {
  const result = useAtomValue(atoms.globalPlaceholdersAtom);
  const retry = useAtomRefresh(atoms.globalPlaceholdersAtom);
  if (AsyncResult.isSuccess(result)) return <GlobalValues globals={result.value} />;
  if (AsyncResult.isFailure(result)) {
    const error = Option.getOrUndefined(Cause.findErrorOption(result.cause));
    const needsRestart = error?._tag === "StorageError" && error.operation === "storage.decode";
    return (
      <Empty>
        <EmptyHeader>
          <EmptyTitle>
            {needsRestart ? "Restart Missal to apply this update" : "Could not load global values"}
          </EmptyTitle>
          <EmptyDescription>
            {needsRestart
              ? "The running app does not yet support global values. Restart Missal, then return to this page."
              : error?._tag === "StorageError"
                ? error.message
                : "The database could not be read. Try again."}
          </EmptyDescription>
        </EmptyHeader>
        {!needsRestart && (
          <EmptyContent>
            <Button variant="outline" size="sm" onClick={retry}>
              Try again
            </Button>
          </EmptyContent>
        )}
      </Empty>
    );
  }
  return <Skeleton className="h-64" />;
}

function GlobalValues({ globals }: { globals: readonly GlobalPlaceholder[] }) {
  const [saved, setSaved] = useState(() => draftsFrom(globals));
  const [rows, setRows] = useState(() => draftsFrom(globals));
  const [saving, setSaving] = useState(false);
  const save = useAtomSet(atoms.saveGlobalPlaceholdersAtom, { mode: "promiseExit" });
  const dirty = JSON.stringify(rows) !== JSON.stringify(saved);

  function change(index: number, property: "label" | "value", value: string) {
    setRows((current) =>
      current.map((row, i) => (i === index ? { ...row, [property]: value } : row)),
    );
  }

  async function submit() {
    if (saving || !dirty) return;
    const decoded = Schema.decodeUnknownExit(SaveGlobalPlaceholdersInput)({ entries: rows });
    if (Exit.isFailure(decoded)) {
      toast.add({ title: "Give every global placeholder a name", type: "error" });
      return;
    }
    setSaving(true);
    try {
      const exit = await save(decoded.value);
      if (Exit.isFailure(exit)) {
        toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
        return;
      }
      const next = draftsFrom(exit.value);
      setRows(next);
      setSaved(next);
      toast.add({ title: "Global values saved", type: "success" });
    } finally {
      setSaving(false);
    }
  }

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <UnsavedChanges isDirty={() => dirty} />
      <p className="text-sm text-muted-foreground">
        Set these once for all FIRs. Use the placeholder in a template; documents fill in its saved
        value.
      </p>
      <div
        className="hidden grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_2rem] gap-3 text-xs text-muted-foreground sm:grid"
        aria-hidden="true"
      >
        <span>Placeholder name</span>
        <span>Global value</span>
        <span />
      </div>
      <FieldGroup className="gap-3">
        {rows.map((row, index) => (
          <div
            key={row._tag === "Existing" ? row.id : `new-${index}`}
            className="grid grid-cols-[minmax(0,1fr)_2rem] items-start gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_2rem] sm:gap-3"
          >
            <Field className="col-start-1">
              <FieldLabel className="sm:sr-only" htmlFor={`global-label-${index}`}>
                Placeholder name
              </FieldLabel>
              <Input
                id={`global-label-${index}`}
                aria-label={`Placeholder name ${index + 1}`}
                lang="ur"
                dir="auto"
                required
                disabled={saving}
                value={row.label}
                placeholder="Name"
                onChange={(event) => change(index, "label", event.target.value)}
              />
            </Field>
            <Field className="col-start-1 sm:col-start-2">
              <FieldLabel className="sm:sr-only" htmlFor={`global-value-${index}`}>
                Global value
              </FieldLabel>
              <Input
                id={`global-value-${index}`}
                aria-label={`Global value for ${row.label || "new placeholder"}`}
                lang="ur"
                dir="auto"
                disabled={saving}
                value={row.value}
                placeholder="Enter a value"
                onChange={(event) => change(index, "value", event.target.value)}
              />
            </Field>
            <div className="col-start-2 row-start-1 sm:col-start-3">
              {row._tag === "Existing" ? (
                <Tooltip>
                  <TooltipTrigger
                    render={
                      <Button
                        type="button"
                        size="icon-sm"
                        variant="ghost"
                        aria-label={`Copy placeholder for ${row.label}`}
                        onClick={() => {
                          void navigator.clipboard.writeText(`@${row.id}@`).then(
                            () =>
                              toast.add({
                                title: "Placeholder copied. Paste it into a template.",
                                type: "success",
                              }),
                            () =>
                              toast.add({
                                title: `Copy this placeholder: @${row.id}@`,
                                type: "info",
                              }),
                          );
                        }}
                      />
                    }
                  >
                    <HugeiconsIcon icon={Copy01Icon} />
                  </TooltipTrigger>
                  <TooltipContent>Copy placeholder @{row.id}@</TooltipContent>
                </Tooltip>
              ) : (
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  disabled={saving}
                  aria-label="Remove new global placeholder"
                  onClick={() => setRows((current) => current.filter((_, i) => i !== index))}
                >
                  <HugeiconsIcon icon={Cancel01Icon} />
                </Button>
              )}
            </div>
          </div>
        ))}
      </FieldGroup>
      <Button
        type="button"
        size="sm"
        variant="ghost"
        className="self-start"
        disabled={saving}
        onClick={() => {
          const index = rows.length;
          setRows((current) => [...current, { _tag: "New", label: "", value: "" }]);
          requestAnimationFrame(() => document.getElementById(`global-label-${index}`)?.focus());
        }}
      >
        <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
        Add global placeholder
      </Button>
      <div className="sticky bottom-0 flex items-center justify-end gap-2 border-t bg-background py-3">
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={!dirty || saving}
          onClick={() => setRows(saved)}
        >
          Reset
        </Button>
        <Button type="submit" size="sm" disabled={!dirty || saving}>
          {saving ? "Saving…" : "Save changes"}
        </Button>
      </div>
    </form>
  );
}

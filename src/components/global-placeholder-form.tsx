import { useState } from "react";
import { useAtomSet, useAtomValue, useAtomRefresh } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Cause, Exit, Option, Schema } from "effect";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "#/components/ui/button";
import { Field, FieldDescription, FieldLabel, FieldLegend, FieldSet } from "#/components/ui/field";
import {
  PlaceholderList,
  PlaceholderListActions,
  PlaceholderToken,
} from "#/components/placeholder-list";
import { useShortcut } from "#/hooks/use-shortcut";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "#/components/ui/input-group";
import {
  Empty,
  EmptyHeader,
  EmptyTitle,
  EmptyDescription,
  EmptyContent,
} from "#/components/ui/empty";
import { Skeleton } from "#/components/ui/skeleton";
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
              ? "The running app does not yet support global values. Restart Missal, then return to Settings."
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
  return (
    <div className="flex flex-col gap-3">
      <Skeleton className="h-4 w-80" />
      <Skeleton className="h-40" />
    </div>
  );
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

  useShortcut("save", () => void submit(), { enabled: dirty && !saving });

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <UnsavedChanges isDirty={() => dirty} />
      <FieldSet>
        <FieldLegend>Global values</FieldLegend>
        <FieldDescription>
          Set once and used by every FIR wherever the placeholder appears in a template, written
          like <PlaceholderToken name="تھانہ نام" />.
        </FieldDescription>
        <PlaceholderList>
          {rows.map((row, index) => (
            <Field key={row._tag === "Existing" ? row.id : `new-${index}`} data-disabled={saving}>
              <FieldLabel className="sr-only" htmlFor={`global-value-${index}`}>
                Value for {row.label || "new placeholder"}
              </FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id={`global-label-${index}`}
                  aria-label={`Placeholder name ${index + 1}`}
                  required
                  disabled={saving}
                  value={row.label}
                  placeholder="نام"
                  onChange={(event) => change(index, "label", event.target.value)}
                />
                <span aria-hidden className="h-4 w-px shrink-0 bg-border" />
                <InputGroupInput
                  id={`global-value-${index}`}
                  disabled={saving}
                  value={row.value}
                  placeholder="قدر درج کریں"
                  onChange={(event) => change(index, "value", event.target.value)}
                />
                {row._tag === "New" ? (
                  <InputGroupAddon align="inline-end">
                    <InputGroupButton
                      size="icon-xs"
                      disabled={saving}
                      aria-label="Remove new global placeholder"
                      onClick={() => setRows((current) => current.filter((_, i) => i !== index))}
                    >
                      <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
                    </InputGroupButton>
                  </InputGroupAddon>
                ) : null}
              </InputGroup>
            </Field>
          ))}
        </PlaceholderList>
        <PlaceholderListActions
          addLabel="Add global value"
          dirty={dirty}
          saving={saving}
          onReset={() => setRows(saved)}
          onAdd={() => {
            const index = rows.length;
            setRows((current) => [...current, { _tag: "New", label: "", value: "" }]);
            requestAnimationFrame(() => document.getElementById(`global-label-${index}`)?.focus());
          }}
        />
      </FieldSet>
    </form>
  );
}

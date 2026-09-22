import { useState } from "react";
import { useAtomSet, useAtomValue, useAtomRefresh } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Cause, Exit, Option, Schema } from "effect";
import { Add01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "#/components/ui/button";
import { Field, FieldLabel } from "#/components/ui/field";
import { PageFooter } from "#/components/page";
import {
  PlaceholderList,
  PlaceholderListFooter,
  PlaceholderListCell,
  PlaceholderListRow,
  PlaceholderListTable,
} from "#/components/placeholder-list";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { useShortcut } from "#/hooks/use-shortcut";
import { Input } from "#/components/ui/input";
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

export function GlobalPlaceholderForm({ active = true }: { active?: boolean }) {
  const result = useAtomValue(atoms.globalPlaceholdersAtom);
  const retry = useAtomRefresh(atoms.globalPlaceholdersAtom);
  if (AsyncResult.isSuccess(result)) return <GlobalValues active={active} globals={result.value} />;
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

function GlobalValues({
  active,
  globals,
}: {
  active: boolean;
  globals: readonly GlobalPlaceholder[];
}) {
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

  useShortcut("save", () => void submit(), { enabled: active && dirty && !saving });

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
        Set these once for all FIRs. Documents use the saved value wherever the placeholder appears
        in a template.
      </p>
      <PlaceholderList>
        <PlaceholderListTable nameHeading="Name" valueHeading="Value">
          {rows.map((row, index) => (
            <PlaceholderListRow key={row._tag === "Existing" ? row.id : `new-${index}`}>
              <PlaceholderListCell>
                <Field data-disabled={saving}>
                  <FieldLabel className="sr-only" htmlFor={`global-label-${index}`}>
                    Placeholder name {index + 1}
                  </FieldLabel>
                  <Input
                    id={`global-label-${index}`}
                    aria-label={`Placeholder name ${index + 1}`}
                    lang="ur"
                    dir="rtl"
                    required
                    disabled={saving}
                    value={row.label}
                    placeholder="متغیر کا نام"
                    onChange={(event) => change(index, "label", event.target.value)}
                  />
                </Field>
              </PlaceholderListCell>
              <PlaceholderListCell>
                <Field data-disabled={saving}>
                  <FieldLabel className="sr-only" htmlFor={`global-value-${index}`}>
                    Global value for {row.label || "new placeholder"}
                  </FieldLabel>
                  <Input
                    id={`global-value-${index}`}
                    aria-label={`Global value for ${row.label || "new placeholder"}`}
                    lang="ur"
                    dir="rtl"
                    disabled={saving}
                    value={row.value}
                    placeholder="قدر درج کریں"
                    onChange={(event) => change(index, "value", event.target.value)}
                  />
                </Field>
              </PlaceholderListCell>
              <PlaceholderListCell>
                {row._tag === "New" ? (
                  <Button
                    type="button"
                    size="icon-sm"
                    variant="ghost"
                    disabled={saving}
                    aria-label="Remove new global placeholder"
                    title="Remove"
                    onClick={() => setRows((current) => current.filter((_, i) => i !== index))}
                  >
                    <HugeiconsIcon icon={Cancel01Icon} />
                  </Button>
                ) : (
                  <span />
                )}
              </PlaceholderListCell>
            </PlaceholderListRow>
          ))}
        </PlaceholderListTable>
        <PlaceholderListFooter>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={saving}
            onClick={() => {
              const index = rows.length;
              setRows((current) => [...current, { _tag: "New", label: "", value: "" }]);
              requestAnimationFrame(() =>
                document.getElementById(`global-label-${index}`)?.focus(),
              );
            }}
          >
            <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
            Add global placeholder
          </Button>
        </PlaceholderListFooter>
      </PlaceholderList>
      <PageFooter>
        <span className="me-auto text-xs text-muted-foreground" role="status">
          {dirty ? "Unsaved changes" : null}
        </span>
        <Button
          type="button"
          variant="ghost"
          disabled={!dirty || saving}
          onClick={() => setRows(saved)}
        >
          Reset
        </Button>
        <Button type="submit" disabled={!dirty || saving}>
          {saving ? "Saving…" : "Save changes"}
          <ShortcutKbd id="save" />
        </Button>
      </PageFooter>
    </form>
  );
}

import { useState } from "react";
import { useAtomSet, useAtomValue, useAtomRefresh } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Cause, Exit, Option, Schema } from "effect";
import { Add01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "#/components/ui/button";
import { Field, FieldLabel } from "#/components/ui/field";
import { Hint } from "#/components/hint";
import { PaneActionsPortal, SaveStatus } from "#/components/pane";
import {
  PlaceholderList,
  PlaceholderListCell,
  PlaceholderListFooter,
  PlaceholderListIntro,
  PlaceholderListRow,
  PlaceholderListTable,
} from "#/components/placeholder-list";
import { Spinner } from "#/components/ui/spinner";
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

const GLOBAL_VALUES_FORM_ID = "global-values-form";
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
  return (
    <div className="flex flex-col gap-3">
      <Skeleton className="h-4 w-80" />
      <Skeleton className="h-64" />
    </div>
  );
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
      id={GLOBAL_VALUES_FORM_ID}
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <UnsavedChanges isDirty={() => dirty} />
      {active ? (
        <PaneActionsPortal>
          {dirty ? <SaveStatus dirty /> : null}
          <Button
            type="button"
            variant="subtle"
            size="sm"
            disabled={!dirty || saving}
            onClick={() => setRows(saved)}
          >
            Reset
          </Button>
          <Hint label="Save" shortcut="save">
            <Button
              type="submit"
              form={GLOBAL_VALUES_FORM_ID}
              size="sm"
              disabled={!dirty || saving}
            >
              {saving ? <Spinner data-icon="inline-start" /> : null}
              Save
            </Button>
          </Hint>
        </PaneActionsPortal>
      ) : null}
      <PlaceholderListIntro>
        Set once and used by every FIR wherever the placeholder appears in a template.
      </PlaceholderListIntro>
      <PlaceholderList>
        <PlaceholderListTable nameHeading="نام" valueHeading="قدر">
          {rows.map((row, index) => (
            <PlaceholderListRow key={row._tag === "Existing" ? row.id : `new-${index}`}>
              <PlaceholderListCell>
                <Field data-disabled={saving}>
                  <FieldLabel className="sr-only" htmlFor={`global-label-${index}`}>
                    Placeholder name {index + 1}
                  </FieldLabel>
                  <Input
                    id={`global-label-${index}`}
                    variant="cell"
                    className="-ms-2.25"
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
                    Value for {row.label || "new placeholder"}
                  </FieldLabel>
                  <Input
                    id={`global-value-${index}`}
                    variant="cell"
                    className="-ms-2.25"
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
                    size="icon-xs"
                    variant="subtle"
                    disabled={saving}
                    aria-label="Remove new global placeholder"
                    onClick={() => setRows((current) => current.filter((_, i) => i !== index))}
                  >
                    <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
                  </Button>
                ) : null}
              </PlaceholderListCell>
            </PlaceholderListRow>
          ))}
        </PlaceholderListTable>
        <PlaceholderListFooter>
          <Button
            type="button"
            size="sm"
            variant="subtle"
            disabled={saving}
            onClick={() => {
              const index = rows.length;
              setRows((current) => [...current, { _tag: "New", label: "", value: "" }]);
              requestAnimationFrame(() =>
                document.getElementById(`global-label-${index}`)?.focus(),
              );
            }}
          >
            <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
            Add global value
          </Button>
        </PlaceholderListFooter>
      </PlaceholderList>
    </form>
  );
}

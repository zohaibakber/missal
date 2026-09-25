import { useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit } from "effect";
import { useNavigate } from "@tanstack/react-router";
import { Add01Icon, LegalDocument01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Hint } from "#/components/hint";
import { ShortcutKbd } from "#/components/shortcut-kbd";
import { Button } from "#/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "#/components/ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "#/components/ui/popover";
import { Spinner } from "#/components/ui/spinner";
import { toast } from "#/components/ui/toast";
import { useShortcut } from "#/hooks/use-shortcut";
import { AddFirTemplatesInput } from "#/lib/fir-document";
import type { FirDocumentId, FirId, TemplateId } from "#/lib/ids";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { atoms } from "#/state/atoms";

type TemplateChecklistProps = {
  firId: FirId;
  onAdded: (id: FirDocumentId) => void;
};

type AddTemplatesPopoverProps = TemplateChecklistProps & {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function AddTemplatesPopover({
  firId,
  onAdded,
  open,
  onOpenChange: setOpen,
}: AddTemplatesPopoverProps) {
  useShortcut("addTemplates", () => setOpen(true));

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Hint label="Add templates" shortcut="addTemplates">
        <PopoverTrigger
          render={
            <Button type="button" variant="ghost" size="icon-sm" aria-label="Add templates" />
          }
        >
          <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
        </PopoverTrigger>
      </Hint>
      <PopoverContent align="start" className="w-80">
        <TemplateChecklist
          firId={firId}
          onAdded={(id) => {
            setOpen(false);
            onAdded(id);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}

function TemplateChecklist({ firId, onAdded }: TemplateChecklistProps) {
  const navigate = useNavigate();
  const result = useAtomValue(atoms.templatesAtom);
  const documentsResult = useAtomValue(atoms.firDocumentsAtom(firId));
  const addTemplates = useAtomSet(atoms.addFirTemplatesAtom, { mode: "promiseExit" });
  const [selected, setSelected] = useState<ReadonlySet<TemplateId>>(() => new Set());
  const [adding, setAdding] = useState(false);
  const attached = new Set(
    AsyncResult.isSuccess(documentsResult)
      ? documentsResult.value.map((document) => document.templateId)
      : [],
  );
  const templates = AsyncResult.isSuccess(result) ? result.value : [];

  function toggle(id: TemplateId) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function addSelected() {
    if (adding || !selected.size) return;
    setAdding(true);
    const exit = await addTemplates(
      new AddFirTemplatesInput({ firId, templateIds: [...selected] }),
    );
    setAdding(false);
    if (Exit.isFailure(exit)) {
      toast.add({ title: getRepositoryErrorMessage(exit), type: "error" });
      return;
    }
    const created = exit.value.find((document) => !attached.has(document.templateId));
    toast.add({
      title: selected.size === 1 ? "Document added" : `${selected.size} documents added`,
      type: "success",
    });
    setSelected(new Set());
    if (created) onAdded(created.id);
  }

  return (
    <Command
      onKeyDown={(event) => {
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          void addSelected();
        }
      }}
    >
      <CommandInput placeholder="Search templates" />
      <CommandList className="max-h-80">
        <CommandEmpty>
          {AsyncResult.isInitial(result) ? "Loading…" : "No templates found."}
        </CommandEmpty>
        {templates.length ? (
          <CommandGroup>
            {templates.map((template) => {
              const isAttached = attached.has(template.id);
              return (
                <CommandItem
                  key={template.id}
                  value={`${template.name} ${template.id}`}
                  disabled={isAttached || adding}
                  data-checked={isAttached || selected.has(template.id)}
                  onSelect={() => toggle(template.id)}
                >
                  <HugeiconsIcon icon={LegalDocument01Icon} strokeWidth={2} />
                  <span lang="ur" dir="rtl" className="min-w-0 flex-1 truncate text-ur">
                    {template.name}
                  </span>
                  {isAttached ? <span className="text-xs text-muted-foreground">Added</span> : null}
                </CommandItem>
              );
            })}
          </CommandGroup>
        ) : null}
      </CommandList>
      <div className="flex items-center gap-2 border-t p-1.5 ps-3">
        {templates.length ? (
          <span className="text-xs text-muted-foreground tabular-nums">
            {selected.size ? `${selected.size} selected` : "Select templates to add"}
          </span>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            className="-ms-1.5"
            onClick={() => void navigate({ to: "/templates/new" })}
          >
            Create a template
          </Button>
        )}
        <Button
          size="sm"
          className="ms-auto"
          disabled={!selected.size || adding}
          onClick={() => void addSelected()}
        >
          {adding ? <Spinner data-icon="inline-start" /> : null}
          Add
          <ShortcutKbd id="confirm" />
        </Button>
      </div>
    </Command>
  );
}

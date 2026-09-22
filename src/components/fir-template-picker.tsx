import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon } from "@hugeicons/core-free-icons";
import { useState } from "react";
import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { AsyncResult } from "effect/unstable/reactivity";
import { Exit } from "effect";
import { Link } from "@tanstack/react-router";
import { Button } from "#/components/ui/button";
import { Checkbox } from "#/components/ui/checkbox";
import { Input } from "#/components/ui/input";
import { SidebarGroup, SidebarGroupContent } from "#/components/ui/sidebar";
import { Empty, EmptyHeader, EmptyTitle, EmptyDescription } from "#/components/ui/empty";
import { Skeleton } from "#/components/ui/skeleton";
import { toast } from "#/components/ui/toast";
import { AddFirTemplatesInput } from "#/lib/fir-document";
import type { FirId, FirDocumentId, TemplateId } from "#/lib/ids";
import { getRepositoryErrorMessage } from "#/lib/storage-errors";
import { atoms } from "#/state/atoms";

export function FirTemplatePicker({
  firId,
  searchId,
  onAdded,
}: {
  firId: FirId;
  searchId?: string;
  onAdded: (id: FirDocumentId) => void;
}) {
  const result = useAtomValue(atoms.templatesAtom);
  const documentsResult = useAtomValue(atoms.firDocumentsAtom(firId));
  const addTemplates = useAtomSet(atoms.addFirTemplatesAtom, { mode: "promiseExit" });
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<ReadonlySet<TemplateId>>(new Set());
  const [adding, setAdding] = useState(false);
  const documents = AsyncResult.isSuccess(documentsResult) ? documentsResult.value : [];
  const attached = new Set(documents.map((document) => document.templateId));
  const templates = AsyncResult.isSuccess(result) ? result.value : [];
  const filtered = templates.filter((template) =>
    `${template.name} ${template.previewText}`
      .toLocaleLowerCase()
      .includes(search.trim().toLocaleLowerCase()),
  );

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
    setSelected(new Set());
    const created = exit.value.find((document) => !attached.has(document.templateId));
    toast.add({ title: "Templates added to this FIR", type: "success" });
    if (created) onAdded(created.id);
  }

  return (
    <SidebarGroup>
      <SidebarGroupContent className="flex flex-col gap-2">
        <div className="flex items-center gap-1">
          <Input
            id={searchId}
            aria-label="Search templates"
            placeholder="Search templates…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && selected.size) {
                event.preventDefault();
                void addSelected();
              }
            }}
          />
          <Button
            variant="ghost"
            size="icon-sm"
            nativeButton={false}
            render={<Link to="/templates/new" />}
            aria-label="Create template"
            title="Create template"
          >
            <HugeiconsIcon icon={Add01Icon} />
          </Button>
        </div>
        <div className="flex flex-col gap-1">
          {AsyncResult.isInitial(result) ? (
            <Skeleton className="h-24" />
          ) : AsyncResult.isFailure(result) ? (
            <p className="px-2 text-sm text-destructive">Could not load templates.</p>
          ) : filtered.length ? (
            filtered.map((template) => (
              <label
                key={template.id}
                className="flex min-h-8 cursor-pointer items-center gap-2 rounded-md px-2 hover:bg-sidebar-accent has-focus-visible:bg-sidebar-accent has-disabled:cursor-default has-disabled:opacity-60"
                title={
                  attached.has(template.id) ? `${template.name} · Already added` : template.name
                }
              >
                <Checkbox
                  aria-label={`Add ${template.name}`}
                  checked={attached.has(template.id) || selected.has(template.id)}
                  disabled={attached.has(template.id) || adding}
                  onCheckedChange={(checked) =>
                    setSelected((current) => {
                      const next = new Set(current);
                      if (checked) next.add(template.id);
                      else next.delete(template.id);
                      return next;
                    })
                  }
                />
                <span className="min-w-0 flex-1">
                  <span lang="ur" dir="rtl" className="block truncate text-base leading-loose">
                    {template.name}
                  </span>
                </span>
              </label>
            ))
          ) : (
            <Empty size="compact">
              <EmptyHeader>
                <EmptyTitle>No templates found</EmptyTitle>
                <EmptyDescription>
                  {search ? "Try another search." : "Create a template to start writing."}
                </EmptyDescription>
              </EmptyHeader>
            </Empty>
          )}
        </div>
        {selected.size > 0 && (
          <Button
            type="button"
            size="sm"
            className="w-full"
            disabled={adding}
            onClick={() => void addSelected()}
          >
            <HugeiconsIcon icon={Add01Icon} data-icon="inline-start" />
            {adding ? "Adding…" : `Add selected (${selected.size})`}
          </Button>
        )}
      </SidebarGroupContent>
    </SidebarGroup>
  );
}

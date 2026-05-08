import { useState } from "react";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { CreateFirForm } from "#/components/create-fir-form";
import { FirTable, FirTableSkeleton } from "#/components/fir-table";
import { Button } from "#/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "#/components/ui/dialog";

export const Route = createFileRoute("/dataset")({
  component: RouteComponent,
});

function RouteComponent() {
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false);

  return (
    <main className="space-y-4 p-4">
      <section className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl font-medium">FIR Dataset</h1>
        </div>

        <Dialog open={isCreateDialogOpen} onOpenChange={setIsCreateDialogOpen}>
          <DialogTrigger render={<Button type="button" />}>Create FIR</DialogTrigger>
          <DialogContent className="max-h-[90vh] overflow-y-auto">
            <DialogHeader>
              <DialogTitle>Create FIR</DialogTitle>
            </DialogHeader>
            <CreateFirForm onSuccess={() => setIsCreateDialogOpen(false)} />
          </DialogContent>
        </Dialog>
      </section>

      <ClientOnly fallback={<FirTableSkeleton />}>
        <FirTable />
      </ClientOnly>
    </main>
  );
}

import { ClientOnly, Link, createFileRoute } from "@tanstack/react-router";
import { FirTable, FirTableSkeleton } from "#/components/fir-table";
import { Button } from "#/components/ui/button";

export const Route = createFileRoute("/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <main className="space-y-4 p-4">
      <section className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl font-medium">FIR Dataset</h1>
        </div>

        <Button nativeButton={false} render={<Link to="/new" />}>
          Create FIR
        </Button>
      </section>

      <ClientOnly fallback={<FirTableSkeleton />}>
        <FirTable />
      </ClientOnly>
    </main>
  );
}

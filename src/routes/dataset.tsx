import { FirTable } from "#/components/fir-table";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/dataset")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <main className="p-4">
      <FirTable />
    </main>
  );
}

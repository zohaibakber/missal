import { createFileRoute } from "@tanstack/react-router";
import { FirTable } from "#/components/fir-table";

export const Route = createFileRoute("/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <main className="flex flex-col gap-3 p-4">
      <FirTable />
    </main>
  );
}

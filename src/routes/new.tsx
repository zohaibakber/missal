import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { CreateFirForm } from "#/components/create-fir-form";

export const Route = createFileRoute("/new")({
  component: RouteComponent,
});

function RouteComponent() {
  const navigate = useNavigate();

  return (
    <main className="p-4">
      <CreateFirForm
        onSuccess={(firId) => {
          void navigate({
            to: "/$firId",
            params: { firId: `${firId}` },
          });
        }}
        className="mx-auto"
      />
    </main>
  );
}

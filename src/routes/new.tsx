import { ClientOnly, createFileRoute, useNavigate } from "@tanstack/react-router";

import { CreateFirForm } from "#/components/create-fir-form";
import { Skeleton } from "#/components/ui/skeleton";

export const Route = createFileRoute("/new")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <ClientOnly fallback={<CreateFirSkeleton />}>
      <CreateFirPage />
    </ClientOnly>
  );
}

function CreateFirPage() {
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

function CreateFirSkeleton() {
  return (
    <main className="flex flex-col gap-4 p-4">
      <div className="flex flex-col gap-2">
        <Skeleton className="h-5 w-40" />
        <Skeleton className="h-7 w-32" />
      </div>
      <Skeleton className="h-[34rem]" />
    </main>
  );
}

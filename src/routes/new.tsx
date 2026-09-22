import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { CreateFirForm } from "#/components/create-fir-form";
import { PageDescription, PageHeader, PageHeading, PageTitle } from "#/components/page";

export const Route = createFileRoute("/new")({
  component: RouteComponent,
});

function RouteComponent() {
  const navigate = useNavigate();

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col">
      <PageHeader className="px-6 pt-6">
        <PageHeading>
          <PageTitle>New FIR</PageTitle>
          <PageDescription>
            Case details fill the placeholders in every document you create for this FIR.
          </PageDescription>
        </PageHeading>
      </PageHeader>
      <CreateFirForm
        onSuccess={(firId) => {
          void navigate({
            to: "/$firId",
            params: { firId: `${firId}` },
          });
        }}
      />
    </div>
  );
}

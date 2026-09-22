import { createFileRoute } from "@tanstack/react-router";
import { FirTable, NewFirButton } from "#/components/fir-table";
import {
  Page,
  PageActions,
  PageDescription,
  PageHeader,
  PageHeading,
  PageTitle,
} from "#/components/page";

export const Route = createFileRoute("/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <Page>
      <PageHeader>
        <PageHeading>
          <PageTitle>FIRs</PageTitle>
          <PageDescription>Open an FIR to write and print its documents.</PageDescription>
        </PageHeading>
        <PageActions>
          <NewFirButton />
        </PageActions>
      </PageHeader>
      <FirTable />
    </Page>
  );
}

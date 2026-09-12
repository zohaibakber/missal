import { createFileRoute } from "@tanstack/react-router";
import { FirNotFound, FirWorkspace } from "#/components/fir-workspace";
import { parseFirId } from "#/lib/ids";

export const Route = createFileRoute("/$firId_/edit")({
  component: RouteComponent,
  validateSearch: (search: Record<string, unknown>) => ({
    documentId: typeof search.documentId === "string" ? search.documentId : undefined,
  }),
});

function RouteComponent() {
  const { firId } = Route.useParams();
  const { documentId } = Route.useSearch();
  const id = parseFirId(firId);

  if (!id) {
    return <FirNotFound />;
  }

  return <FirWorkspace key={id} documentId={documentId} firId={id} />;
}

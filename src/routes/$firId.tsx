import { createFileRoute } from "@tanstack/react-router";
import { FirNotFound, FirWorkspace } from "#/components/fir-workspace";
import { parseFirId } from "#/lib/ids";

export const Route = createFileRoute("/$firId")({
  component: RouteComponent,
  validateSearch: (search: Record<string, unknown>): { documentId?: string } => ({
    documentId: typeof search.documentId === "string" ? search.documentId : undefined,
  }),
});

function RouteComponent() {
  const { firId } = Route.useParams();
  const { documentId } = Route.useSearch();
  const id = parseFirId(firId);
  return id ? <FirWorkspace key={id} documentId={documentId} firId={id} /> : <FirNotFound />;
}

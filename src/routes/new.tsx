import { createFileRoute, useNavigate, useRouter } from "@tanstack/react-router";
import { FirForm, FirFormFields, FirFormStatus, FirFormSubmit } from "#/components/fir-form";
import { Pane, PaneActions, PaneBody, PaneHeader, PaneTitle } from "#/components/pane";
import { Button } from "#/components/ui/button";

export const Route = createFileRoute("/new")({
  component: RouteComponent,
});

function RouteComponent() {
  const navigate = useNavigate();
  const router = useRouter();

  return (
    <FirForm
      className="flex h-full min-h-0 flex-col"
      onSuccess={(firId) => {
        void navigate({ to: "/$firId", params: { firId: `${firId}` }, replace: true });
      }}
    >
      <Pane>
        <PaneHeader>
          <PaneTitle>New FIR</PaneTitle>
          <PaneActions>
            <FirFormStatus />
            <Button type="button" variant="subtle" size="sm" onClick={() => router.history.back()}>
              Cancel
            </Button>
            <FirFormSubmit>Create FIR</FirFormSubmit>
          </PaneActions>
        </PaneHeader>
        <PaneBody>
          <div className="mx-auto w-full max-w-2xl px-6 py-10">
            <FirFormFields />
          </div>
        </PaneBody>
      </Pane>
    </FirForm>
  );
}

import { createFileRoute } from "@tanstack/react-router";
import { SignIn } from "@clerk/tanstack-react-start";

export const Route = createFileRoute("/sign-in/$")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <main className="flex flex-col h-dvh items-center justify-center w-full">
      <SignIn />
    </main>
  );
}

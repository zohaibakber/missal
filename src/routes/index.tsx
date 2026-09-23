import { createFileRoute } from "@tanstack/react-router";
import { FirsPane } from "#/components/fir-table";

export const Route = createFileRoute("/")({
  component: FirsPane,
});

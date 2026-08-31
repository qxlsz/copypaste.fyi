import { createFileRoute } from "@tanstack/react-router";
import { PasteView } from "@/components/paste-view";

export const Route = createFileRoute("/raw/$id")({
  component: RawPastePage,
});

function RawPastePage() {
  const { id } = Route.useParams();
  return <PasteView id={id} raw />;
}

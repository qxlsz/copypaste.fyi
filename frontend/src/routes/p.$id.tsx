import { createFileRoute } from "@tanstack/react-router";
import { PasteView } from "@/components/paste-view";

export const Route = createFileRoute("/p/$id")({
  component: PastePage,
});

function PastePage() {
  const { id } = Route.useParams();
  return <PasteView id={id} />;
}

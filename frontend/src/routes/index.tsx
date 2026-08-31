import { createFileRoute } from "@tanstack/react-router";
import { Composer } from "@/components/composer";

export const Route = createFileRoute("/")({ component: Home });

function Home() {
  return <Composer />;
}

import { createFileRoute } from "@tanstack/react-router";
import { corsPreflight, json } from "@/lib/http";
import { discoveryDocument } from "@/lib/protocol";

export const Route = createFileRoute("/api/v1")({
  server: {
    handlers: {
      OPTIONS: async () => corsPreflight(),
      GET: async ({ request }) => {
        const origin = new URL(request.url).origin;
        return json(discoveryDocument(origin));
      },
    },
  },
});

import { createFileRoute } from "@tanstack/react-router";
import { allowRequest, clientIp } from "@/lib/abuse";
import { corsPreflight, json, rateLimited } from "@/lib/http";
import { fetchPaste } from "@/lib/paste-store";

export const Route = createFileRoute("/api/v1/pastes/$id")({
  server: {
    handlers: {
      OPTIONS: async () => corsPreflight(),
      GET: async ({ params, request }) => {
        const allowed = allowRequest(clientIp(request), "read");
        if (!allowed.ok) return rateLimited(allowed.retryAfter);
        const result = await fetchPaste(params.id);
        if (result.status === "ok") return json(result);
        if (result.status === "blocked") return json({ status: "not_found" }, 404);
        const status = result.status === "not_found" ? 404 : 410;
        return json(result, status);
      },
    },
  },
});

import { createFileRoute } from "@tanstack/react-router";
import { allowRequest, clientIp } from "@/lib/abuse";
import { corsPreflight, json, rateLimited, text } from "@/lib/http";
import { fetchPaste } from "@/lib/paste-store";

export const Route = createFileRoute("/api/v1/pastes/$id/raw")({
  server: {
    handlers: {
      OPTIONS: async () => corsPreflight(),
      GET: async ({ params, request }) => {
        const allowed = allowRequest(clientIp(request), "read");
        if (!allowed.ok) return rateLimited(allowed.retryAfter);
        const result = await fetchPaste(params.id);
        if (result.status !== "ok") {
          if (result.status === "blocked") return text("not found\n", 404);
          const status = result.status === "not_found" ? 404 : 410;
          return text(`${result.status}\n`, status);
        }
        if (result.paste.encrypted) {
          return json(
            {
              status: "encrypted",
              hint: "Fetch JSON from /api/v1/pastes/{id} and decrypt client-side. Ciphertext is not raw text.",
              id: result.paste.id,
            },
            409,
          );
        }
        return text(result.paste.content, 200, {
          "content-disposition": `inline; filename="paste-${result.paste.id.slice(0, 8)}.txt"`,
        });
      },
    },
  },
});

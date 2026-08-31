import { createFileRoute } from "@tanstack/react-router";
import { allowRequest, checkWriteToken, clientIp } from "@/lib/abuse";
import { corsPreflight, json, rateLimited } from "@/lib/http";
import { insertPaste } from "@/lib/paste-store";

export const Route = createFileRoute("/api/v1/pastes")({
  server: {
    handlers: {
      OPTIONS: async () => corsPreflight(),
      POST: async ({ request }) => {
        if (!checkWriteToken(request)) {
          return json({ error: "write_token_required" }, 401);
        }
        const ip = clientIp(request);
        const allowed = allowRequest(ip, "create");
        if (!allowed.ok) return rateLimited(allowed.retryAfter);

        let body: Record<string, unknown>;
        const contentType = request.headers.get("content-type") || "";
        try {
          if (contentType.includes("application/json")) {
            body = (await request.json()) as Record<string, unknown>;
          } else {
            const textBody = await request.text();
            body = { content: textBody };
          }
        } catch {
          return json({ error: "invalid_body" }, 400);
        }

        try {
          const created = await insertPaste(
            {
              content: String(body.content ?? ""),
              format: String(body.format ?? "plain_text"),
              encrypted: Boolean(body.encrypted),
              algorithm: body.algorithm == null ? null : String(body.algorithm),
              salt: body.salt == null ? null : String(body.salt),
              nonce: body.nonce == null ? null : String(body.nonce),
              burnAfterReading: Boolean(body.burnAfterReading ?? body.burn),
              retentionMinutes:
                body.retentionMinutes == null && body.ttl == null
                  ? undefined
                  : Number(body.retentionMinutes ?? body.ttl),
            },
            "api",
          );
          const origin = new URL(request.url).origin;
          return json(
            {
              id: created.id,
              url: `${origin}${created.path}`,
              raw: `${origin}${created.raw}`,
              expiresAt: created.expiresAt,
            },
            201,
          );
        } catch (error) {
          const message = error instanceof Error ? error.message : "create_failed";
          return json({ error: "invalid_paste", message }, 400);
        }
      },
    },
  },
});

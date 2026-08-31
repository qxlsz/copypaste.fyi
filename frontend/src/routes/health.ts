import { createFileRoute } from "@tanstack/react-router";
import { json, text } from "@/lib/http";
import { PROTOCOL, PROTOCOL_VERSION } from "@/lib/protocol";

export const Route = createFileRoute("/health")({
  server: {
    handlers: {
      GET: async () =>
        json({ ok: true, protocol: PROTOCOL, version: PROTOCOL_VERSION }),
      HEAD: async () => text("", 200),
    },
  },
});

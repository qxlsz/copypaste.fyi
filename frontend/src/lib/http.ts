import { PROTOCOL_HEADERS } from "@/lib/protocol";

const CORS_HEADERS = {
  "access-control-allow-origin": "*",
  "access-control-allow-headers":
    "content-type, authorization, x-write-token, x-copypaste-write-token",
  "access-control-allow-methods": "GET, POST, OPTIONS",
} as const;

export function json(
  body: unknown,
  status = 200,
  extra: HeadersInit = {},
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...PROTOCOL_HEADERS,
      ...CORS_HEADERS,
      ...extra,
    },
  });
}

export function text(body: string, status = 200, extra: HeadersInit = {}): Response {
  return new Response(body, {
    status,
    headers: {
      "content-type": "text/plain; charset=utf-8",
      ...PROTOCOL_HEADERS,
      "access-control-allow-origin": "*",
      ...extra,
    },
  });
}

export function corsPreflight(): Response {
  return new Response(null, {
    status: 204,
    headers: {
      ...PROTOCOL_HEADERS,
      ...CORS_HEADERS,
      "access-control-max-age": "600",
    },
  });
}

export function rateLimited(retryAfter: number): Response {
  return json(
    { error: "rate_limited", retryAfter },
    429,
    { "retry-after": String(retryAfter) },
  );
}

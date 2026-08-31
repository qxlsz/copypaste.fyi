import { timingSafeEqual } from "node:crypto";
import {
  RATE_CREATE_LIMIT,
  RATE_READ_LIMIT,
  RATE_WINDOW_MS,
} from "@/lib/protocol";

type Bucket = { start: number; creates: number; reads: number };

const buckets = new Map<string, Bucket>();

function bucket(ip: string): Bucket {
  const now = Date.now();
  const current = buckets.get(ip);
  if (!current || now - current.start >= RATE_WINDOW_MS) {
    const next = { start: now, creates: 0, reads: 0 };
    buckets.set(ip, next);
    if (buckets.size > 10_000) {
      for (const [key, value] of buckets) {
        if (now - value.start >= RATE_WINDOW_MS) buckets.delete(key);
      }
    }
    return next;
  }
  return current;
}

export function allowRequest(
  ip: string,
  kind: "create" | "read",
): { ok: true } | { ok: false; retryAfter: number } {
  const row = bucket(ip || "local");
  const count = kind === "create" ? row.creates : row.reads;
  const limit = kind === "create" ? RATE_CREATE_LIMIT : RATE_READ_LIMIT;
  if (count >= limit) {
    const retryAfter = Math.max(1, Math.ceil((RATE_WINDOW_MS - (Date.now() - row.start)) / 1000));
    return { ok: false, retryAfter };
  }
  if (kind === "create") row.creates += 1;
  else row.reads += 1;
  return { ok: true };
}

export function looksBinary(bytes: Uint8Array): boolean {
  const sample = bytes.subarray(0, Math.min(bytes.length, 1024));
  let nulls = 0;
  for (const value of sample) if (value === 0) nulls += 1;
  return nulls > 8;
}

export function writeToken(): string | undefined {
  const value = typeof process !== "undefined" ? process.env.COPYPASTE_WRITE_TOKEN : undefined;
  return value && value.trim() ? value.trim() : undefined;
}

export function checkWriteToken(request: Request): boolean {
  const expected = writeToken();
  if (!expected) return true;
  const bearer = request.headers.get("authorization")?.replace(/^Bearer\s+/i, "") ?? "";
  const header =
    request.headers.get("x-write-token") ??
    request.headers.get("x-copypaste-write-token") ??
    "";
  const got = bearer || header;
  return safeEqual(got, expected);
}

function safeEqual(a: string, b: string): boolean {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  if (left.length !== right.length) {
    timingSafeEqual(right, right);
    return false;
  }
  return timingSafeEqual(left, right);
}

export function clientIp(request: Request): string {
  const forwarded = request.headers.get("x-forwarded-for");
  if (forwarded) return forwarded.split(",")[0]?.trim() || "local";
  return request.headers.get("x-real-ip")?.trim() || "local";
}

export function blockedIds(): Set<string> {
  const raw = typeof process !== "undefined" ? process.env.COPYPASTE_BLOCKED_PASTE_IDS : undefined;
  if (!raw) return new Set();
  return new Set(
    raw
      .split(",")
      .map((id) => id.trim())
      .filter(Boolean),
  );
}

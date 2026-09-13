import { MAX_PASTE_BYTES, utf8Bytes } from "./composer";

const SHARE_KEYS = ["text", "title", "url"] as const;

const clip = (value: string): string => {
  if (utf8Bytes(value) <= MAX_PASTE_BYTES) return value;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const bytes = encoder.encode(value);
  return decoder.decode(bytes.slice(0, MAX_PASTE_BYTES));
};

/** Build composer seed text from Web Share Target query params. */
export const shareTargetText = (search: string): string | null => {
  const params = new URLSearchParams(search.startsWith("?") ? search.slice(1) : search);
  const parts: string[] = [];
  for (const key of SHARE_KEYS) {
    const value = params.get(key)?.trim();
    if (value) parts.push(value);
  }
  if (parts.length === 0) return null;
  return clip(Array.from(new Set(parts)).join("\n\n"));
};

export const stripShareTargetSearch = (search: string): string => {
  const params = new URLSearchParams(search.startsWith("?") ? search.slice(1) : search);
  for (const key of SHARE_KEYS) {
    params.delete(key);
  }
  const next = params.toString();
  return next ? `?${next}` : "";
};

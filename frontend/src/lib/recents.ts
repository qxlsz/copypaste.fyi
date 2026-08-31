const STORAGE_KEY = "copypaste.recents.v1";
const MAX_RECENTS = 40;

export type RecentPaste = {
  id: string;
  format: string;
  encrypted: boolean;
  burnAfterReading: boolean;
  createdAt: string;
  expiresAt: string | null;
  preview: string;
};

function isRecent(item: unknown): item is RecentPaste {
  if (!item || typeof item !== "object") return false;
  const row = item as RecentPaste;
  return typeof row.id === "string" && typeof row.preview === "string";
}

function readAll(): RecentPaste[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isRecent);
  } catch {
    return [];
  }
}

function writeAll(items: RecentPaste[]) {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(items.slice(0, MAX_RECENTS)));
}

function notExpired(item: RecentPaste, now = Date.now()): boolean {
  if (!item.expiresAt) return true;
  const expires = new Date(item.expiresAt).getTime();
  return Number.isFinite(expires) && expires > now;
}

export function listRecents(): RecentPaste[] {
  const all = readAll();
  const items = all.filter((item) => notExpired(item));
  if (items.length !== all.length && typeof window !== "undefined") writeAll(items);
  return items;
}

export function rememberPaste(item: RecentPaste) {
  if (typeof window === "undefined") return;
  const next = [item, ...readAll().filter((entry) => entry.id !== item.id && notExpired(entry))];
  writeAll(next);
}

export function forgetPaste(id: string) {
  if (typeof window === "undefined") return;
  writeAll(readAll().filter((entry) => entry.id !== id));
}

export function clearRecents() {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(STORAGE_KEY);
}

export function previewFrom(content: string): string {
  const compact = content.replace(/\s+/g, " ").trim();
  if (!compact) return "Empty paste";
  return compact.length > 80 ? `${compact.slice(0, 77)}…` : compact;
}

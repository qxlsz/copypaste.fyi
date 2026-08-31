const ID_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

export const PASTE_ID_LENGTH = 24;

// 62 * 4 = 248. Reject 248–255 so `byte % 62` is unbiased.
const REJECT_ABOVE = ID_ALPHABET.length * 4;

export function generatePasteId(): string {
  const out: string[] = [];
  const bytes = new Uint8Array(32);
  while (out.length < PASTE_ID_LENGTH) {
    crypto.getRandomValues(bytes);
    for (let i = 0; i < bytes.length && out.length < PASTE_ID_LENGTH; i += 1) {
      const byte = bytes[i];
      if (byte >= REJECT_ABOVE) continue;
      out.push(ID_ALPHABET[byte % ID_ALPHABET.length]);
    }
  }
  return out.join("");
}

export function isUniqueViolation(error: unknown): boolean {
  if (!error || typeof error !== "object") return false;
  const code = "code" in error ? String(error.code) : "";
  if (code === "23505") return true;
  const message = error instanceof Error ? error.message : String(error);
  return /duplicate key|unique constraint|already exists/i.test(message);
}

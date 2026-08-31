const encoder = new TextEncoder();
const decoder = new TextDecoder();

function bytesToB64url(bytes: Uint8Array): string {
  const chunk = 0x8000;
  let binary = "";
  for (let i = 0; i < bytes.length; i += chunk) {
    const slice = bytes.subarray(i, i + chunk);
    binary += String.fromCharCode(...slice);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function b64urlToBytes(value: string): Uint8Array {
  const padded =
    value.length % 4 === 0 ? value : value + "=".repeat(4 - (value.length % 4));
  const binary = atob(padded.replace(/-/g, "+").replace(/_/g, "/"));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export function generateEncryptionKey(): string {
  return bytesToB64url(crypto.getRandomValues(new Uint8Array(32)));
}

export function validateEncryptionKey(key: string): string | null {
  if (!key.trim()) return "An encryption key is required.";
  if (key.length < 8) return "Use at least 8 characters, or generate a key.";
  if (encoder.encode(key).byteLength > 1024) {
    return "Encryption keys must be 1024 bytes or smaller.";
  }
  return null;
}

async function deriveAesKey(secret: string, salt: Uint8Array): Promise<CryptoKey> {
  const material = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    "PBKDF2",
    false,
    ["deriveKey"],
  );
  return crypto.subtle.deriveKey(
    {
      name: "PBKDF2",
      salt: salt as BufferSource,
      iterations: 210_000,
      hash: "SHA-256",
    },
    material,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

export async function encryptPaste(plaintext: string, secret: string) {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveAesKey(secret, salt);
  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: nonce },
    key,
    encoder.encode(plaintext),
  );
  return {
    content: bytesToB64url(new Uint8Array(ciphertext)),
    salt: bytesToB64url(salt),
    nonce: bytesToB64url(nonce),
    algorithm: "aes256_gcm" as const,
  };
}

export async function decryptPaste(input: {
  content: string;
  salt: string;
  nonce: string;
  secret: string;
}): Promise<string> {
  const salt = b64urlToBytes(input.salt);
  const nonce = b64urlToBytes(input.nonce);
  const ciphertext = b64urlToBytes(input.content);
  const key = await deriveAesKey(input.secret, salt);
  const plaintext = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: nonce as BufferSource },
    key,
    ciphertext as BufferSource,
  );
  return decoder.decode(plaintext);
}

export function readKeyFromHash(): string {
  if (typeof window === "undefined") return "";
  const raw = window.location.hash.replace(/^#/, "");
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
}

export function buildShareUrl(path: string, key?: string): string {
  const origin = typeof window === "undefined" ? "" : window.location.origin;
  const url = `${origin}${path}`;
  if (!key) return url;
  return `${url}#${encodeURIComponent(key)}`;
}

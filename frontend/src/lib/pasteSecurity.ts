export const ENCRYPTION_KEY_BYTES = 32;
export const MAX_ENCRYPTION_KEY_BYTES = 1024;
export const MAX_STEGO_IMAGE_BYTES = 1024 * 1024;

const PNG_SIGNATURE = new Uint8Array([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
]);
const BMP_SIGNATURE = new Uint8Array([0x42, 0x4d]);
const BASE64_CHUNK_BYTES = 0x8000;

export type SupportedStegoMime = "image/png" | "image/bmp";

export class StegoImageValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "StegoImageValidationError";
  }
}

const hasSignature = (bytes: Uint8Array, signature: Uint8Array) =>
  bytes.length >= signature.length &&
  signature.every((byte, index) => bytes[index] === byte);

const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += BASE64_CHUNK_BYTES) {
    binary += String.fromCharCode(
      ...bytes.subarray(offset, offset + BASE64_CHUNK_BYTES),
    );
  }
  return btoa(binary);
};

export const generateEncryptionKey = (): string => {
  const bytes = new Uint8Array(ENCRYPTION_KEY_BYTES);
  globalThis.crypto.getRandomValues(bytes);
  return bytesToBase64(bytes)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/u, "");
};

export const validateEncryptionKey = (key: string): string | null => {
  if (!key.trim()) {
    return "Encryption requires a non-empty key.";
  }
  if (new TextEncoder().encode(key).byteLength > MAX_ENCRYPTION_KEY_BYTES) {
    return `Encryption keys must be ${MAX_ENCRYPTION_KEY_BYTES} bytes or smaller.`;
  }
  return null;
};

export const buildPasteShareUrl = (
  shareableUrl: string,
  encryptionKey: string,
  frontendOrigin: string,
): string => {
  const expectedOrigin = new URL(frontendOrigin).origin;
  const url = new URL(shareableUrl, expectedOrigin);
  if (
    url.origin !== expectedOrigin ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    !/^\/p\/[A-Za-z0-9_-]+$/u.test(url.pathname)
  ) {
    throw new Error("The server returned an unsafe paste share URL.");
  }
  if (encryptionKey.trim()) {
    url.hash = `key=${encodeURIComponent(encryptionKey)}`;
  }
  return url.toString();
};

export const detectStegoImageMime = (
  bytes: Uint8Array,
): SupportedStegoMime => {
  if (hasSignature(bytes, PNG_SIGNATURE)) return "image/png";
  if (hasSignature(bytes, BMP_SIGNATURE)) return "image/bmp";
  throw new StegoImageValidationError(
    "Only genuine PNG or BMP carrier images are supported.",
  );
};

export const readStegoImage = async (
  file: File,
): Promise<{ dataUri: string; mime: SupportedStegoMime }> => {
  if (file.size > MAX_STEGO_IMAGE_BYTES) {
    throw new StegoImageValidationError(
      "Carrier images must be 1 MiB or smaller.",
    );
  }

  const bytes = new Uint8Array(await file.arrayBuffer());
  const mime = detectStegoImageMime(bytes);
  return {
    dataUri: `data:${mime};base64,${bytesToBase64(bytes)}`,
    mime,
  };
};

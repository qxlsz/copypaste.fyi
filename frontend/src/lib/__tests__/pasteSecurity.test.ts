import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ENCRYPTION_KEY_BYTES,
  MAX_ENCRYPTION_KEY_BYTES,
  MAX_STEGO_IMAGE_BYTES,
  buildPasteShareUrl,
  detectStegoImageMime,
  generateEncryptionKey,
  readStegoImage,
  validateEncryptionKey,
} from "../pasteSecurity";

const decodeBase64Url = (value: string) => {
  const base64 = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = base64.padEnd(Math.ceil(base64.length / 4) * 4, "=");
  return Uint8Array.from(atob(padded), (character) => character.charCodeAt(0));
};

afterEach(() => {
  vi.restoreAllMocks();
});

describe("generateEncryptionKey", () => {
  it("generates a 256-bit base64url key without Math.random", () => {
    const mathRandom = vi
      .spyOn(Math, "random")
      .mockImplementation(() => {
        throw new Error("Math.random must not generate encryption keys");
      });

    const key = generateEncryptionKey();

    expect(key).toMatch(/^[A-Za-z0-9_-]{43}$/u);
    expect(decodeBase64Url(key)).toHaveLength(ENCRYPTION_KEY_BYTES);
    expect(mathRandom).not.toHaveBeenCalled();
  });
});

describe("validateEncryptionKey", () => {
  it("rejects empty and whitespace-only keys", () => {
    expect(validateEncryptionKey("")).toBe(
      "Encryption requires a non-empty key.",
    );
    expect(validateEncryptionKey(" \n\t ")).toBe(
      "Encryption requires a non-empty key.",
    );
  });

  it("enforces the limit in UTF-8 bytes", () => {
    expect(validateEncryptionKey("a".repeat(MAX_ENCRYPTION_KEY_BYTES))).toBe(
      null,
    );
    expect(
      validateEncryptionKey("é".repeat(MAX_ENCRYPTION_KEY_BYTES / 2 + 1)),
    ).toBe(
      `Encryption keys must be ${MAX_ENCRYPTION_KEY_BYTES} bytes or smaller.`,
    );
  });
});

describe("buildPasteShareUrl", () => {
  it("creates an encrypted SPA share link with the key only in the fragment", () => {
    const url = buildPasteShareUrl(
      "/p/secure-paste_42",
      "key?/with sensitive characters",
      "https://www.copypaste.fyi",
    );

    expect(url).toBe(
      "https://www.copypaste.fyi/p/secure-paste_42#key=key%3F%2Fwith%20sensitive%20characters",
    );
    expect(url.split("#", 1)[0]).not.toContain("key");
  });

  it("rejects cross-origin, legacy, and query-bearing response links", () => {
    for (const unsafe of [
      "https://evil.example/p/id",
      "/legacy-id",
      "/p/id?key=secret",
      "//evil.example/p/id",
    ]) {
      expect(() =>
        buildPasteShareUrl(unsafe, "", "https://www.copypaste.fyi"),
      ).toThrow("unsafe paste share URL");
    }
  });
});

describe("steganographic carrier validation", () => {
  it("recognizes PNG and BMP magic bytes instead of trusting MIME metadata", () => {
    expect(
      detectStegoImageMime(
        new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
      ),
    ).toBe("image/png");
    expect(detectStegoImageMime(new Uint8Array([0x42, 0x4d, 0, 0]))).toBe(
      "image/bmp",
    );
    expect(() =>
      detectStegoImageMime(new Uint8Array([0x47, 0x49, 0x46, 0x38])),
    ).toThrow("Only genuine PNG or BMP");
  });

  it("rejects an oversized file before reading it", async () => {
    const arrayBuffer = vi.fn<() => Promise<ArrayBuffer>>();
    const oversizedFile = {
      size: MAX_STEGO_IMAGE_BYTES + 1,
      arrayBuffer,
    } as unknown as File;

    await expect(readStegoImage(oversizedFile)).rejects.toThrow(
      "1 MiB or smaller",
    );
    expect(arrayBuffer).not.toHaveBeenCalled();
  });

  it("normalizes the data URI MIME from the validated signature", async () => {
    const bytes = new Uint8Array([
      0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    ]);
    const file = {
      size: bytes.byteLength,
      type: "image/svg+xml",
      arrayBuffer: vi.fn(async () => bytes.buffer),
    } as unknown as File;

    await expect(readStegoImage(file)).resolves.toEqual({
      dataUri: "data:image/png;base64,iVBORw0KGgo=",
      mime: "image/png",
    });
  });
});

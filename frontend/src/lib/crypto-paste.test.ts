import assert from "node:assert/strict";
import { test } from "node:test";
import {
  decryptPaste,
  encryptPaste,
  generateEncryptionKey,
  validateEncryptionKey,
} from "./crypto-paste.ts";

test("validateEncryptionKey rejects empty and tiny secrets", () => {
  assert.equal(validateEncryptionKey(""), "An encryption key is required.");
  assert.equal(validateEncryptionKey("short"), "Use at least 8 characters, or generate a key.");
  assert.equal(validateEncryptionKey("long-enough-secret"), null);
});

test("encryptPaste / decryptPaste round-trip unicode", async () => {
  const secret = generateEncryptionKey();
  const plaintext = "hello — 你好 — 🔐\nsecond line";
  const encrypted = await encryptPaste(plaintext, secret);
  assert.equal(encrypted.algorithm, "aes256_gcm");
  assert.ok(encrypted.content.length > 0);
  const decoded = await decryptPaste({ ...encrypted, secret });
  assert.equal(decoded, plaintext);
});

test("decryptPaste fails with the wrong secret", async () => {
  const encrypted = await encryptPaste("secret note", generateEncryptionKey());
  await assert.rejects(() =>
    decryptPaste({ ...encrypted, secret: "wrong-wrong" }),
  );
});

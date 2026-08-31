import assert from "node:assert/strict";
import { test } from "node:test";
import { generatePasteId, isUniqueViolation, PASTE_ID_LENGTH } from "./paste-id.ts";

test("generatePasteId returns a 24-char alphanumeric id", () => {
  const id = generatePasteId();
  assert.equal(id.length, PASTE_ID_LENGTH);
  assert.match(id, /^[A-Za-z0-9]{24}$/);
});

test("generatePasteId is unique across many draws", () => {
  const seen = new Set<string>();
  for (let i = 0; i < 400; i += 1) seen.add(generatePasteId());
  assert.equal(seen.size, 400);
});

test("isUniqueViolation detects postgres 23505 and message fallbacks", () => {
  assert.equal(isUniqueViolation({ code: "23505" }), true);
  assert.equal(isUniqueViolation(new Error("duplicate key value violates unique constraint")), true);
  assert.equal(isUniqueViolation(new Error("boom")), false);
  assert.equal(isUniqueViolation(null), false);
});

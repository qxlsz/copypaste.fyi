import assert from "node:assert/strict";
import { test } from "node:test";
import { DEFAULT_API_TTL_MINUTES, PROTOCOL, discoveryDocument } from "./protocol.ts";

test("discovery document is agent-readable", () => {
  const doc = discoveryDocument("https://example.test");
  assert.equal(doc.protocol, PROTOCOL);
  assert.equal(doc.defaultTtlMinutes, DEFAULT_API_TTL_MINUTES);
  assert.equal(doc.ethics.listing, false);
  assert.match(doc.endpoints.create, /\/api\/v1\/pastes$/);
  assert.match(doc.endpoints.tools, /\/tools\.json$/);
  assert.match(doc.agent.serve, /copypaste serve/);
  assert.match(doc.ethics.writeAdmission, /X-CopyPaste-Write-Token/);
});

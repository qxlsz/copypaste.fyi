import { describe, expect, it } from "vitest";

import { agentReceipt } from "../agent";

describe("agentReceipt", () => {
  it("gives another agent the URL without leaking a key when there is none", () => {
    const parsed = JSON.parse(agentReceipt("https://www.copypaste.fyi/p/abc123")) as Record<
      string,
      unknown
    >;
    expect(parsed.copypaste).toBe(1);
    expect(parsed.id).toBe("abc123");
    expect(parsed.get).toBe("https://www.copypaste.fyi/api/pastes/abc123");
    expect(parsed.key).toBeUndefined();
    expect(parsed.headers).toBeUndefined();
  });

  it("puts the decryption token in headers, never in the URL", () => {
    const url = "https://www.copypaste.fyi/p/secret01";
    const key = "super-secret-token";
    const parsed = JSON.parse(agentReceipt(url, key, "aes256_gcm")) as {
      url: string;
      key: string;
      headers: Record<string, string>;
    };
    expect(parsed.url).toBe(url);
    expect(parsed.url.includes(key)).toBe(false);
    expect(parsed.key).toBe(key);
    expect(parsed.headers["X-Paste-Key"]).toBe(key);
  });
});

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  createPaste,
  fetchPaste,
  fetchUserPastes,
  loginWithSignature,
  logoutUser,
  rawPasteUrl,
} from "../client";

const mockFetch = vi.fn();

beforeEach(() => {
  mockFetch.mockReset();
  vi.stubGlobal("fetch", mockFetch);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function makeResponse(
  status: number,
  body: string,
  ok = status >= 200 && status < 300,
) {
  return {
    ok,
    status,
    statusText: "Error",
    text: () => Promise.resolve(body),
    json: () => Promise.resolve(JSON.parse(body)),
  };
}

describe("jsonFetch error sanitization", () => {
  it("throws generic message for 500 errors, not raw server body", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(
        500,
        '{"error":"internal panic at src/server/handlers.rs:42","stack":"..."}',
        false,
      ),
    );

    await expect(
      createPaste({ content: "test", format: "plain_text" }),
    ).rejects.toThrow("Something went wrong. Please try again later.");
  });

  it("throws generic message for 502 errors", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(502, "<html>Bad Gateway</html>", false),
    );

    await expect(
      createPaste({ content: "test", format: "plain_text" }),
    ).rejects.toThrow("Something went wrong. Please try again later.");
  });

  it("uses structured message for 4xx with conforming API error body", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(
        422,
        '{"code":"validation_error","message":"Content exceeds maximum length"}',
        false,
      ),
    );

    await expect(
      createPaste({ content: "test", format: "plain_text" }),
    ).rejects.toThrow("Content exceeds maximum length");
  });

  it("shows generic message for 4xx with non-JSON body", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(400, "Bad request - field foo missing", false),
    );

    await expect(
      createPaste({ content: "test", format: "plain_text" }),
    ).rejects.toThrow("Request failed (400)");
  });

  it("shows generic message for 4xx with JSON missing code field", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(400, '{"message":"some internal detail"}', false),
    );

    await expect(
      createPaste({ content: "test", format: "plain_text" }),
    ).rejects.toThrow("Request failed (400)");
  });

  it("shows generic message for 4xx with JSON missing message field", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(400, '{"code":"bad_request"}', false),
    );

    await expect(
      createPaste({ content: "test", format: "plain_text" }),
    ).rejects.toThrow("Request failed (400)");
  });

  it("turns a create 401 into a clear write-credential error", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(
        401,
        '{"code":"unauthorized","message":"Unauthorized"}',
        false,
      ),
    );

    await expect(
      createPaste({ content: "test", format: "plain_text" }),
    ).rejects.toMatchObject({
      status: 401,
      code: "write_credential_required",
      message: "Posting requires an operator-issued write credential.",
    });
  });
});

describe("jsonFetch CSRF header", () => {
  it("sends X-Requested-With header on POST requests", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(
        200,
        '{"id":"abc","path":"/p/abc","shareableUrl":"http://x/p/abc","isLive":false}',
      ),
    );

    await createPaste({ content: "test", format: "plain_text" });

    const [, init] = mockFetch.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers["X-Requested-With"]).toBe("XMLHttpRequest");
  });

  it("sends X-Requested-With header on login POST", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(200, '{"token":"tok","pubkeyHash":"hash"}'),
    );

    await loginWithSignature("challenge", "sig", "pubkey");

    const [, init] = mockFetch.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers["X-Requested-With"]).toBe("XMLHttpRequest");
  });
});

describe("sensitive request handling", () => {
  it("builds raw links on the API origin without putting secrets in them", () => {
    const url = rawPasteUrl("paste/id");

    expect(new URL(url).pathname).toBe("/raw/paste%2Fid");
    expect(url).not.toContain("key");
  });

  it("omits credentials and rejects redirects without a bearer token", async () => {
    mockFetch
      .mockResolvedValueOnce(
        makeResponse(
          200,
          '{"id":"abc","path":"/p/abc","shareableUrl":"http://x/p/abc","isLive":false}',
        ),
      )
      .mockResolvedValueOnce(
        makeResponse(200, '{"token":"tok","pubkeyHash":"hash"}'),
      );

    await createPaste({ content: "sensitive", format: "plain_text" });
    await loginWithSignature("challenge", "sig", "pubkey");

    for (const [, init] of mockFetch.mock.calls as [string, RequestInit][]) {
      expect(init.credentials).toBe("omit");
      expect(init.redirect).toBe("error");
      expect(init.cache).toBe("no-store");
    }
  });

  it("attaches a session bearer only on authenticated API requests", async () => {
    mockFetch
      .mockResolvedValueOnce(
        makeResponse(
          200,
          '{"id":"abc","path":"/p/abc","shareableUrl":"http://x/p/abc","isLive":false}',
        ),
      )
      .mockResolvedValueOnce(makeResponse(200, '{"pastes":[]}'))
      .mockResolvedValueOnce(makeResponse(200, '{"success":true}'));

    await createPaste(
      { content: "test", format: "plain_text" },
      { sessionToken: "session-token" },
    );
    await fetchUserPastes("owner-hash", "session-token");
    await logoutUser("session-token");

    for (const [, init] of mockFetch.mock.calls as [string, RequestInit][]) {
      const headers = init.headers as Record<string, string>;
      expect(headers.Authorization).toBe("Bearer session-token");
      expect(init.credentials).toBe("omit");
      expect(init.redirect).toBe("error");
      expect(init.cache).toBe("no-store");
    }
  });

  it("keeps service admission separate from user-session identity", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(
        200,
        '{"id":"abc","path":"/p/abc","shareableUrl":"/p/abc","isLive":false}',
      ),
    );

    await createPaste(
      { content: "test", format: "plain_text" },
      {
        sessionToken: "user-session",
        writeCredential: "operator-admission",
      },
    );

    const [, init] = mockFetch.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers.Authorization).toBe("Bearer user-session");
    expect(headers["X-CopyPaste-Write-Token"]).toBe("operator-admission");
  });

  it("does not attach a bearer to unauthenticated paste reads", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(
        200,
        '{"id":"paste-id","format":"plain_text","content":"public","createdAt":1,"burnAfterReading":false,"bundle":null,"encryption":{"algorithm":"none","requiresKey":false},"torAccessOnly":false}',
      ),
    );

    await fetchPaste("paste-id");

    const [, init] = mockFetch.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers).not.toHaveProperty("Authorization");
  });

  it("sends paste keys only in X-Paste-Key and disables caching", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(
        200,
        '{"id":"paste-id","format":"plain_text","content":"secret","createdAt":1,"burnAfterReading":false,"bundle":null,"encryption":{"algorithm":"aes256_gcm","requiresKey":true},"torAccessOnly":false}',
      ),
    );

    await fetchPaste("paste id", "key?/with sensitive characters");

    const [url, init] = mockFetch.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(url).toMatch(/\/pastes\/paste%20id$/u);
    expect(url).not.toContain("key");
    expect(url).not.toContain("sensitive");
    expect(headers["X-Paste-Key"]).toBe("key?/with sensitive characters");
    expect(init.cache).toBe("no-store");
    expect(init.credentials).toBe("omit");
    expect(init.redirect).toBe("error");
  });

  it("omits X-Paste-Key when no key is supplied", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(
        200,
        '{"id":"paste-id","format":"plain_text","content":"public","createdAt":1,"burnAfterReading":false,"bundle":null,"encryption":{"algorithm":"none","requiresKey":false},"torAccessOnly":false}',
      ),
    );

    await fetchPaste("paste-id");

    const [, init] = mockFetch.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers).not.toHaveProperty("X-Paste-Key");
    expect(init.cache).toBe("no-store");
  });
});

describe("jsonFetch timeout", () => {
  it("throws timeout error when request is aborted", async () => {
    mockFetch.mockImplementationOnce(
      (_url: string, opts: RequestInit) =>
        new Promise<never>((_resolve, reject) => {
          if (opts.signal) {
            opts.signal.addEventListener("abort", () => {
              const err = new Error("The operation was aborted.");
              err.name = "AbortError";
              reject(err);
            });
          }
        }),
    );

    vi.useFakeTimers();
    const promise = createPaste({ content: "test", format: "plain_text" });
    vi.advanceTimersByTime(10_001);

    await expect(promise).rejects.toThrow("timed out");
    vi.useRealTimers();
  });
});

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createPaste, loginWithSignature } from "../client";

const mockFetch = vi.fn();

beforeEach(() => {
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
});

describe("jsonFetch CSRF header", () => {
  it("sends X-Requested-With header on POST requests", async () => {
    mockFetch.mockResolvedValueOnce(
      makeResponse(200, '{"id":"abc","path":"/p/abc","shareableUrl":"http://x/p/abc","isLive":false}'),
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

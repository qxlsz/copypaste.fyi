import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAuth } from "../auth";
import type { User } from "../auth";

const mockUser: User = {
  pubkeyHash: "abc123",
  pubkey: "pubkey_base64",
  privkey: "SECRET_PRIVATE_KEY_MUST_NOT_PERSIST",
  createdAt: 1_000_000,
};

const createMemoryStorage = (): Storage => {
  const entries = new Map<string, string>();
  return {
    get length() {
      return entries.size;
    },
    clear: () => entries.clear(),
    getItem: (key) => entries.get(key) ?? null,
    key: (index) => Array.from(entries.keys())[index] ?? null,
    removeItem: (key) => entries.delete(key),
    setItem: (key, value) => entries.set(key, value),
  };
};

beforeEach(() => {
  vi.stubGlobal("localStorage", createMemoryStorage());
  useAuth.setState({ user: null, token: null, isLoading: false });
});

afterEach(() => {
  vi.unstubAllGlobals();
  useAuth.setState({ user: null, token: null, isLoading: false });
});

describe("memory-only authentication", () => {
  it("does not persist the user, private key, or bearer token", () => {
    useAuth.setState({ user: mockUser, token: "session-token" });

    expect(useAuth.getState().token).toBe("session-token");
    expect(localStorage.getItem("auth-storage")).toBeNull();
  });

  it("removes legacy persisted credentials on module initialization", async () => {
    localStorage.setItem(
      "auth-storage",
      JSON.stringify({ state: { user: mockUser, token: "legacy-token" } }),
    );
    vi.resetModules();

    const { useAuth: freshAuthStore } = await import("../auth");

    expect(localStorage.getItem("auth-storage")).toBeNull();
    expect(freshAuthStore.getState().user).toBeNull();
    expect(freshAuthStore.getState().token).toBeNull();
  });

  it("awaits authenticated server logout before clearing memory", async () => {
    let resolveResponse: ((response: Response) => void) | undefined;
    const fetchMock = vi.fn<
      (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
    >(
      () =>
        new Promise<Response>((resolve) => {
          resolveResponse = resolve;
        }),
    );
    vi.stubGlobal("fetch", fetchMock);
    useAuth.setState({ user: mockUser, token: "session-token" });

    const logoutPromise = useAuth.getState().logout();
    await Promise.resolve();

    expect(useAuth.getState().token).toBe("session-token");
    const [, init] = fetchMock.mock.calls[0];
    expect((init?.headers as Record<string, string>).Authorization).toBe(
      "Bearer session-token",
    );

    resolveResponse?.(
      new Response('{"success":true}', {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    await logoutPromise;

    expect(useAuth.getState().user).toBeNull();
    expect(useAuth.getState().token).toBeNull();
  });
});

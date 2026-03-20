import { describe, expect, it } from "vitest";
import { partializeAuthState } from "../auth";
import type { AuthState, User } from "../auth";

const mockUser: User = {
  pubkeyHash: "abc123",
  pubkey: "pubkey_base64",
  privkey: "SECRET_PRIVATE_KEY_MUST_NOT_PERSIST",
  createdAt: 1_000_000,
};

describe("partializeAuthState", () => {
  it("is a function (not undefined)", () => {
    expect(typeof partializeAuthState).toBe("function");
  });

  it("excludes privkey from persisted state", () => {
    const result = partializeAuthState({ user: mockUser, token: "tok" } as AuthState);
    expect(result.user).not.toHaveProperty("privkey");
  });

  it("preserves pubkeyHash, pubkey, and createdAt", () => {
    const result = partializeAuthState({ user: mockUser, token: "tok" } as AuthState);
    expect(result.user).toEqual({
      pubkeyHash: "abc123",
      pubkey: "pubkey_base64",
      createdAt: 1_000_000,
    });
  });

  it("persists token", () => {
    const result = partializeAuthState({ user: mockUser, token: "my-token" } as AuthState);
    expect(result.token).toBe("my-token");
  });

  it("handles null user", () => {
    const result = partializeAuthState({ user: null, token: null } as AuthState);
    expect(result.user).toBeNull();
    expect(result.token).toBeNull();
  });
});

import {
  createContext,
  useCallback,
  useContext,
  useState,
  type ReactNode,
} from "react";
import * as ed25519 from "@noble/ed25519";
import {
  fetchAuthChallenge,
  loginWithSignature,
  logoutUser,
} from "../api/client";
import { toast } from "sonner";

export interface User {
  pubkeyHash: string;
  pubkey: string;
  privkey: string;
  createdAt: number;
}

interface AuthContextValue {
  user: User | null;
  token: string | null;
  isLoading: boolean;
  login: (privkey?: string) => Promise<void>;
  logout: () => void;
  generateKeys: () => Promise<{ pubkey: string; privkey: string }>;
}

const AUTH_STORAGE_KEY = "auth-storage";

function loadAuthState(): { user: User | null; token: string | null } {
  try {
    const stored = localStorage.getItem(AUTH_STORAGE_KEY);
    if (!stored) return { user: null, token: null };
    const data = JSON.parse(stored) as Record<string, unknown>;
    // Support legacy zustand persist format
    if (data.state && typeof data.state === "object") {
      const s = data.state as Record<string, unknown>;
      return {
        user: (s.user as User | null) ?? null,
        token: (s.token as string | null) ?? null,
      };
    }
    return {
      user: (data.user as User | null) ?? null,
      token: (data.token as string | null) ?? null,
    };
  } catch {
    return { user: null, token: null };
  }
}

function saveAuthState(user: User | null, token: string | null) {
  try {
    localStorage.setItem(AUTH_STORAGE_KEY, JSON.stringify({ user, token }));
  } catch {
    // localStorage unavailable
  }
}

const AuthContext = createContext<AuthContextValue | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [{ user, token }, setAuth] = useState(() => loadAuthState());
  const [isLoading, setIsLoading] = useState(false);

  const generateKeys = useCallback(async () => {
    const privkeyBytes = ed25519.utils.randomPrivateKey();
    const pubkeyBytes = await ed25519.getPublicKey(privkeyBytes);
    return {
      pubkey: btoa(String.fromCharCode(...pubkeyBytes)),
      privkey: btoa(String.fromCharCode(...privkeyBytes)),
    };
  }, []);

  const login = useCallback(async (privkeyParam?: string) => {
    setIsLoading(true);
    try {
      if (
        window.location.protocol !== "https:" &&
        window.location.hostname !== "localhost"
      ) {
        throw new Error("HTTPS is required for cryptographic operations");
      }

      const privkeyBytes = privkeyParam
        ? new Uint8Array(
            atob(privkeyParam)
              .split("")
              .map((c) => c.charCodeAt(0)),
          )
        : ed25519.utils.randomPrivateKey();

      const pubkeyBytes = await ed25519.getPublicKey(privkeyBytes);
      const pubkey = btoa(String.fromCharCode(...pubkeyBytes));

      const { challenge } = await fetchAuthChallenge();
      const challengeBytes = new TextEncoder().encode(challenge);
      const signatureBytes = await ed25519.sign(challengeBytes, privkeyBytes);
      const signature = btoa(String.fromCharCode(...signatureBytes));

      const { token: newToken, pubkeyHash } = await loginWithSignature(
        challenge,
        signature,
        pubkey,
      );

      setAuth((prev) => {
        const newUser: User = {
          pubkeyHash,
          pubkey,
          privkey: btoa(String.fromCharCode(...privkeyBytes)),
          createdAt: prev.user?.createdAt ?? Date.now(),
        };
        saveAuthState(newUser, newToken);
        return { user: newUser, token: newToken };
      });
      toast.success("Logged in successfully");
    } catch (error) {
      const msg = error instanceof Error ? error.message : "Unknown error";
      toast.error("Login failed", { description: msg });
      throw error;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const logout = useCallback(() => {
    logoutUser().catch(() => {});
    setAuth({ user: null, token: null });
    saveAuthState(null, null);
    toast.success("Logged out");
  }, []);

  return (
    <AuthContext.Provider
      value={{ user, token, isLoading, login, logout, generateKeys }}
    >
      {children}
    </AuthContext.Provider>
  );
}

// eslint-disable-next-line react-refresh/only-export-components
export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}

import {
  createContext,
  useContext,
  useState,
  useEffect,
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

export type KeyFormat = "hex" | "base64" | "pem" | "raw";

interface AuthContextValue {
  user: User | null;
  token: string | null;
  isLoading: boolean;
  login: (privkey?: string) => Promise<void>;
  logout: () => void;
  generateKeys: () => Promise<{ pubkey: string; privkey: string }>;
  importKey: (
    keyData: string,
    format: KeyFormat,
  ) => Promise<{ pubkey: string; privkey: string }>;
  validateKeyPair: (privkey: string, pubkey: string) => Promise<boolean>;
}

const STORAGE_KEY = "auth-storage";

function loadFromStorage(): { user: User | null; token: string | null } {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored) {
      return JSON.parse(stored) as { user: User | null; token: string | null };
    }
  } catch {
    // localStorage unavailable
  }
  return { user: null, token: null };
}

const AuthContext = createContext<AuthContextValue | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(() => loadFromStorage().user);
  const [token, setToken] = useState<string | null>(
    () => loadFromStorage().token,
  );
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify({ user, token }));
    } catch {
      // localStorage unavailable
    }
  }, [user, token]);

  const generateKeys = async (): Promise<{
    pubkey: string;
    privkey: string;
  }> => {
    const privkeyBytes = ed25519.utils.randomPrivateKey();
    const pubkeyBytes = await ed25519.getPublicKey(privkeyBytes);
    return {
      pubkey: btoa(String.fromCharCode(...pubkeyBytes)),
      privkey: btoa(String.fromCharCode(...privkeyBytes)),
    };
  };

  const validateKeyPair = async (
    privkeyStr: string,
    pubkeyStr: string,
  ): Promise<boolean> => {
    try {
      const privkeyBytes = new Uint8Array(
        atob(privkeyStr)
          .split("")
          .map((c) => c.charCodeAt(0)),
      );
      const pubkeyBytes = new Uint8Array(
        atob(pubkeyStr)
          .split("")
          .map((c) => c.charCodeAt(0)),
      );
      const derivedPubkey = await ed25519.getPublicKey(privkeyBytes);
      return derivedPubkey.every((byte, i) => byte === pubkeyBytes[i]);
    } catch {
      return false;
    }
  };

  const importKey = async (
    keyData: string,
    format: KeyFormat,
  ): Promise<{ pubkey: string; privkey: string }> => {
    let privkeyBytes: Uint8Array;

    switch (format) {
      case "hex": {
        const hexData = keyData.replace(/^0x/, "");
        if (!/^[0-9a-fA-F]{64}$/.test(hexData)) {
          throw new Error(
            "Invalid hex format - must be 64 characters (32 bytes)",
          );
        }
        privkeyBytes = new Uint8Array(
          hexData.match(/.{2}/g)!.map((byte) => parseInt(byte, 16)),
        );
        break;
      }
      case "base64":
      case "raw": {
        try {
          privkeyBytes = new Uint8Array(
            atob(keyData)
              .split("")
              .map((c) => c.charCodeAt(0)),
          );
        } catch {
          throw new Error(`Invalid ${format} format`);
        }
        break;
      }
      case "pem": {
        const pemMatch = keyData.match(
          /-----BEGIN (?:.* )?PRIVATE KEY-----([\s\S]*?)-----END (?:.* )?PRIVATE KEY-----/,
        );
        if (!pemMatch) {
          throw new Error("Invalid PEM format - expected Ed25519 private key");
        }
        const pemBody = pemMatch[1].replace(/\s/g, "");
        try {
          const derBytes = new Uint8Array(
            atob(pemBody)
              .split("")
              .map((c) => c.charCodeAt(0)),
          );
          if (derBytes.length < 48) throw new Error("PEM key too short");
          privkeyBytes = derBytes.slice(16, 48);
        } catch {
          throw new Error("Failed to decode PEM body");
        }
        break;
      }
      default:
        throw new Error(`Unsupported key format: ${format}`);
    }

    if (privkeyBytes.length !== 32) {
      throw new Error(
        `Invalid key length: ${privkeyBytes.length} bytes (expected 32)`,
      );
    }

    const pubkeyBytes = await ed25519.getPublicKey(privkeyBytes);
    return {
      pubkey: btoa(String.fromCharCode(...pubkeyBytes)),
      privkey: btoa(String.fromCharCode(...privkeyBytes)),
    };
  };

  const login = async (privkeyArg?: string): Promise<void> => {
    setIsLoading(true);
    try {
      if (
        window.location.protocol !== "https:" &&
        window.location.hostname !== "localhost"
      ) {
        throw new Error("HTTPS is required for cryptographic operations");
      }

      const privkeyBytes = privkeyArg
        ? new Uint8Array(
            atob(privkeyArg)
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

      const newUser: User = {
        pubkeyHash,
        pubkey,
        privkey: btoa(String.fromCharCode(...privkeyBytes)),
        createdAt: user?.createdAt ?? Date.now(),
      };
      setUser(newUser);
      setToken(newToken);
      toast.success("Logged in successfully");
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : "Unknown error";
      toast.error("Login failed", { description: errorMessage });
      throw error;
    } finally {
      setIsLoading(false);
    }
  };

  const logout = () => {
    logoutUser().catch(() => {});
    setUser(null);
    setToken(null);
    toast.success("Logged out");
  };

  return (
    <AuthContext.Provider
      value={{
        user,
        token,
        isLoading,
        login,
        logout,
        generateKeys,
        importKey,
        validateKeyPair,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

// eslint-disable-next-line react-refresh/only-export-components
export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) throw new Error("useAuth must be used within AuthProvider");
  return context;
}

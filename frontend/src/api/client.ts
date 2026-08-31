import type {
  CreatePastePayload,
  CreatePasteResponse,
  StatsSummary,
  AuthChallengeResponse,
  UserPasteListResponse,
} from "./types";
import type { PasteViewResponse } from "../server/types";

// In development, use relative /api paths (proxied by Vite)
// In production, use the configured API base
export const API_BASE = import.meta.env.DEV
  ? "/api"
  : (import.meta.env.VITE_API_BASE ?? "/api");

const requestOrigin = (url: string): string => {
  const frontendOrigin = globalThis.location?.origin ?? "http://localhost";
  return new URL(url, frontendOrigin).origin;
};

const guardedApiHeaderOptions = (
  url: string,
  headers: Record<string, string>,
): Pick<RequestInit, "credentials" | "headers" | "redirect"> => {
  if (requestOrigin(url) !== requestOrigin(API_BASE)) {
    throw new Error("Refusing to send secrets outside the API origin");
  }

  return {
    credentials: "omit",
    headers,
    redirect: "error",
  };
};

// Bearer credentials are opt-in per endpoint and are never attached unless the
// request targets the exact origin configured for the API. This prevents a
// future absolute-URL refactor from accidentally disclosing a live session.
const sessionRequestOptions = (
  url: string,
  token?: string | null,
): Pick<RequestInit, "credentials" | "headers" | "redirect"> => {
  if (!token) return {};
  return guardedApiHeaderOptions(url, { Authorization: `Bearer ${token}` });
};

// Typed error thrown for non-2xx API responses so callers can branch on
// status / machine-readable code instead of matching message strings.
export class ApiError extends Error {
  status: number;
  code?: string;

  constructor(message: string, status: number, code?: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

const jsonFetch = async <T>(
  input: RequestInfo,
  init?: RequestInit,
): Promise<T> => {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 10000); // 10 second timeout

  try {
    const response = await fetch(input, {
      ...init,
      // API requests may contain paste bodies, login proofs, or decryption
      // material even when they do not carry an Authorization header. Never
      // attach ambient browser credentials and never replay them through an
      // HTTP redirect.
      credentials: "omit",
      redirect: "error",
      headers: {
        "Content-Type": "application/json",
        "X-Requested-With": "XMLHttpRequest",
        ...(init?.headers ?? {}),
      },
      cache: "no-store",
      signal: controller.signal,
    });
    clearTimeout(timeoutId);

    if (!response.ok) {
      const body = await response.text().catch(() => "");
      if (response.status >= 500) {
        if (import.meta.env.DEV) {
          console.error(`[API] ${response.status} error:`, body);
        }
        throw new ApiError(
          "Something went wrong. Please try again later.",
          response.status,
        );
      }
      // For 4xx: use structured API error message if schema conforms {code, message}
      let userMessage = `Request failed (${response.status})`;
      let code: string | undefined;
      try {
        const parsed = JSON.parse(body) as Record<string, unknown>;
        if (
          typeof parsed.message === "string" &&
          typeof parsed.code === "string"
        ) {
          userMessage = parsed.message;
          code = parsed.code;
        }
      } catch {
        // body is not JSON — use generic message
      }
      throw new ApiError(userMessage, response.status, code);
    }

    if (response.status === 204) {
      return undefined as T;
    }

    return (await response.json()) as T;
  } catch (error) {
    clearTimeout(timeoutId);
    if (error instanceof Error && error.name === "AbortError") {
      throw new Error(
        "Request timed out. Please check if the backend is running.",
      );
    }
    throw error;
  }
};

export const createPaste = async (
  payload: CreatePastePayload,
  credentials?: {
    sessionToken?: string | null;
    writeCredential?: string | null;
  },
): Promise<CreatePasteResponse> => {
  const url = `${API_BASE}/pastes`;
  const credentialHeaders: Record<string, string> = {};
  if (credentials?.sessionToken) {
    credentialHeaders.Authorization = `Bearer ${credentials.sessionToken}`;
  }
  if (credentials?.writeCredential) {
    credentialHeaders["X-CopyPaste-Write-Token"] =
      credentials.writeCredential;
  }
  try {
    return await jsonFetch<CreatePasteResponse>(url, {
      ...(Object.keys(credentialHeaders).length > 0
        ? guardedApiHeaderOptions(url, credentialHeaders)
        : undefined),
      method: "POST",
      body: JSON.stringify(payload),
    });
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      throw new ApiError(
        "Posting requires an operator-issued write credential.",
        401,
        "write_credential_required",
      );
    }
    throw error;
  }
};

export const fetchStatsSummary = async (): Promise<StatsSummary> => {
  const url = `${API_BASE}/stats/summary`;
  return jsonFetch<StatsSummary>(url);
};

export const fetchPaste = async (
  id: string,
  key?: string,
): Promise<PasteViewResponse> => {
  const url = `${API_BASE}/pastes/${encodeURIComponent(id)}`;
  return jsonFetch<PasteViewResponse>(url, {
    ...(key ? guardedApiHeaderOptions(url, { "X-Paste-Key": key }) : undefined),
  });
};

export const rawPasteUrl = (id: string): string => {
  const frontendOrigin = globalThis.location?.origin ?? "http://localhost";
  const apiDirectory = new URL(`${API_BASE.replace(/\/$/, "")}/`, frontendOrigin);
  return new URL(`../raw/${encodeURIComponent(id)}`, apiDirectory).toString();
};

export const fetchAuthChallenge = async (): Promise<AuthChallengeResponse> => {
  const url = `${API_BASE}/auth/challenge`;
  return jsonFetch<AuthChallengeResponse>(url);
};

export const fetchUserPasteCount = async (
  pubkeyHash: string,
  token?: string | null,
): Promise<{ pasteCount: number }> => {
  const url = `${API_BASE}/user/paste-count?pubkey_hash=${encodeURIComponent(pubkeyHash)}`;
  return jsonFetch<{ pasteCount: number }>(url, {
    ...sessionRequestOptions(url, token),
  });
};

export const fetchUserPastes = async (
  pubkeyHash: string,
  token?: string | null,
): Promise<UserPasteListResponse> => {
  const url = `${API_BASE}/user/pastes?pubkey_hash=${encodeURIComponent(pubkeyHash)}`;
  return jsonFetch<UserPasteListResponse>(url, {
    ...sessionRequestOptions(url, token),
  });
};

export const loginWithSignature = async (
  challenge: string,
  signature: string,
  pubkey: string,
): Promise<{ token: string; pubkeyHash: string }> => {
  const url = `${API_BASE}/auth/login`;
  return jsonFetch<{ token: string; pubkeyHash: string }>(url, {
    method: "POST",
    body: JSON.stringify({ challenge, signature, pubkey }),
  });
};

export const logoutUser = async (
  token?: string | null,
): Promise<{ success: boolean }> => {
  const url = `${API_BASE}/auth/logout`;
  return jsonFetch<{ success: boolean }>(url, {
    ...sessionRequestOptions(url, token),
    method: "POST",
    body: JSON.stringify({}),
  });
};

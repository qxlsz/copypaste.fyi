import { pasteIdFromShareUrl } from "./shareImage";

export interface AgentReceipt {
  copypaste: 1;
  url: string;
  id: string;
  get: string;
  algorithm?: string;
  key?: string;
  headers?: Record<string, string>;
}

/** Tokens another agent needs. The share URL without the key stays unreadable. */
export const agentReceipt = (url: string, key?: string, algorithm?: string): string => {
  const id = pasteIdFromShareUrl(url);
  const get = url.includes("/p/") ? url.replace("/p/", "/api/pastes/") : url;
  const receipt: AgentReceipt = {
    copypaste: 1,
    url,
    id,
    get,
  };
  if (key) {
    receipt.algorithm = algorithm || "aes256_gcm";
    receipt.key = key;
    receipt.headers = { "X-Paste-Key": key };
  }
  return JSON.stringify(receipt);
};

export const parseAgentReceipt = (text: string): AgentReceipt | null => {
  const raw = text.trim();
  if (!raw.startsWith("{") || raw.length > 8192) return null;
  try {
    const value = JSON.parse(raw) as Partial<AgentReceipt>;
    if (value.copypaste !== 1) return null;
    if (typeof value.url !== "string" && typeof value.get !== "string") return null;
    const url = typeof value.url === "string" ? value.url : String(value.get);
    const id = typeof value.id === "string" ? value.id : pasteIdFromShareUrl(url);
    return {
      copypaste: 1,
      url,
      id,
      get: typeof value.get === "string" ? value.get : url,
      algorithm: value.algorithm,
      key: value.key,
      headers: value.headers,
    };
  } catch {
    return null;
  }
};

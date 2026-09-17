import { publicPasteUrl } from "./openAgents";
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
  const clean = publicPasteUrl(url);
  const id = pasteIdFromShareUrl(clean);
  const get = clean.includes("/p/") ? clean.replace("/p/", "/api/pastes/") : clean;
  const receipt: AgentReceipt = {
    copypaste: 1,
    url: clean,
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

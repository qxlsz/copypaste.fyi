/** Drop fragments and query so Open-with URLs cannot leak a paste key. */
export const publicPasteUrl = (url: string): string => {
  try {
    const parsed = new URL(url);
    parsed.hash = "";
    parsed.search = "";
    return parsed.toString();
  } catch {
    return url.split("#")[0]?.split("?")[0] ?? url;
  }
};

/** Prompt handed to another model. Never put an encryption key in this URL. */
export const openPrompt = (url: string): string =>
  `Read this copypaste.fyi paste and continue the work. If you need the protocol, fetch /.well-known/copypaste.json from the same origin.\n\n${publicPasteUrl(url)}`;

export type OpenAgentId = "grok" | "codex" | "chatgpt" | "claude";

export interface OpenAgent {
  id: OpenAgentId;
  label: string;
  web: (prompt: string) => string;
  androidPackage?: string;
  iosScheme?: string;
}

export const OPEN_AGENTS: OpenAgent[] = [
  {
    id: "grok",
    label: "Grok",
    web: (prompt) => `https://grok.com/?q=${encodeURIComponent(prompt)}`,
    androidPackage: "ai.x.grok",
    iosScheme: "grok",
  },
  {
    id: "codex",
    label: "Codex",
    web: (prompt) => `https://chatgpt.com/?q=${encodeURIComponent(`Use Codex. ${prompt}`)}`,
    androidPackage: "com.openai.chatgpt",
    iosScheme: "chatgpt",
  },
  {
    id: "chatgpt",
    label: "ChatGPT",
    web: (prompt) => `https://chatgpt.com/?q=${encodeURIComponent(prompt)}`,
    androidPackage: "com.openai.chatgpt",
    iosScheme: "chatgpt",
  },
  {
    id: "claude",
    label: "Claude",
    web: (prompt) => `https://claude.ai/new?q=${encodeURIComponent(prompt)}`,
    androidPackage: "com.anthropic.claude",
    iosScheme: "claude",
  },
];

export const isAndroidUa = (ua: string): boolean => /Android/i.test(ua);

export const isIosUa = (ua: string): boolean => /iPhone|iPad|iPod/i.test(ua);

const androidIntent = (httpsUrl: string, pkg: string): string => {
  const rest = httpsUrl.replace(/^https:\/\//, "");
  return `intent://${rest}#Intent;scheme=https;package=${pkg};S.browser_fallback_url=${encodeURIComponent(httpsUrl)};end`;
};

/** HTTPS on desktop. Android tries the store app, then the same HTTPS URL. */
export const agentHref = (agent: OpenAgent, prompt: string, ua = ""): string => {
  const web = agent.web(prompt);
  if (isAndroidUa(ua) && agent.androidPackage) {
    return androidIntent(web, agent.androidPackage);
  }
  return web;
};

export const agentSchemeHref = (agent: OpenAgent, prompt: string): string | null => {
  if (!agent.iosScheme) return null;
  return `${agent.iosScheme}://?q=${encodeURIComponent(prompt)}`;
};

export const parseOpenAgentId = (value: string | null): OpenAgentId | null => {
  if (value === "grok" || value === "codex" || value === "chatgpt" || value === "claude") {
    return value;
  }
  return null;
};

/** Share URL that opens this paste in an agent. Never includes #key=. */
export const pasteOpenUrl = (url: string, agent: OpenAgentId): string => {
  const clean = publicPasteUrl(url);
  const parsed = new URL(clean, "https://www.copypaste.fyi");
  parsed.searchParams.set("open", agent);
  return parsed.toString();
};

export const launchAgent = (agent: OpenAgent, prompt: string, ua = navigator.userAgent): void => {
  const web = agent.web(prompt);
  if (isIosUa(ua)) {
    const scheme = agentSchemeHref(agent, prompt);
    if (scheme) {
      window.location.href = scheme;
      window.setTimeout(() => {
        if (document.visibilityState === "visible") {
          window.location.href = web;
        }
      }, 700);
      return;
    }
  }
  window.location.href = agentHref(agent, prompt, ua);
};

export const grokBotHref = (prompt: string, ua = ""): string => {
  const grok = OPEN_AGENTS.find((agent) => agent.id === "grok");
  if (!grok) return `https://grok.com/?q=${encodeURIComponent(prompt)}`;
  return agentHref(grok, prompt, ua);
};

export const GROK_BOT_ADD_PROMPT =
  "Install this copypaste.fyi Grok Bot from my clipboard. Confirm you can POST /api/pastes and GET /api/pastes/{id} with X-Paste-Key when encrypted. Do not put keys in URLs.";

/** Skill text to paste into Grok as a custom bot / teammate. */
export const GROK_BOT_SKILL = `# copypaste.fyi Grok Bot

You send and read pastes on https://www.copypaste.fyi

## Send
POST https://www.copypaste.fyi/api/pastes
Content-Type: application/json
{"content":"<text>","format":"plain_text"}

Closed instances: header X-CopyPaste-Write-Token.

## Read
GET https://www.copypaste.fyi/api/pastes/{id}
Encrypted: header X-Paste-Key
Raw: GET /raw/{id}

## Rules
- Type → Get link → share. There is no public listing.
- Missing, burned, and expired reads are the same 404.
- Never put tokens in argv, query strings, or chat URLs.
- Discovery: https://www.copypaste.fyi/.well-known/copypaste.json
- Long form: https://www.copypaste.fyi/llms.txt
`;

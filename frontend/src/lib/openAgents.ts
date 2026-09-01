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

export interface OpenAgent {
  id: "grok" | "codex" | "chatgpt" | "claude";
  label: string;
  href: (prompt: string) => string;
}

export const OPEN_AGENTS: OpenAgent[] = [
  {
    id: "grok",
    label: "Grok",
    href: (prompt) => `https://grok.com/?q=${encodeURIComponent(prompt)}`,
  },
  {
    id: "codex",
    label: "Codex",
    href: (prompt) => `https://chatgpt.com/?q=${encodeURIComponent(`Use Codex. ${prompt}`)}`,
  },
  {
    id: "chatgpt",
    label: "ChatGPT",
    href: (prompt) => `https://chatgpt.com/?q=${encodeURIComponent(prompt)}`,
  },
  {
    id: "claude",
    label: "Claude",
    href: (prompt) => `https://claude.ai/new?q=${encodeURIComponent(prompt)}`,
  },
];

export const grokBotHref = (prompt: string): string =>
  `https://grok.com/?q=${encodeURIComponent(prompt)}`;

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

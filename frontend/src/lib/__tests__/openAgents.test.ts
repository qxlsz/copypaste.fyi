import { describe, expect, it } from "vitest";

import { GROK_BOT_SKILL, OPEN_AGENTS, grokBotHref, openPrompt } from "../openAgents";

const URL = "https://www.copypaste.fyi/p/secret01";
const KEY = "super-secret-token-do-not-leak";

describe("openAgents", () => {
  it("never puts an encryption key in a third-party chat URL", () => {
    const prompt = openPrompt(URL);
    expect(prompt).toContain(URL);
    expect(prompt.toLowerCase()).not.toContain("key=");
    for (const agent of OPEN_AGENTS) {
      const href = agent.href(prompt);
      expect(href).not.toContain(KEY);
      expect(decodeURIComponent(href)).not.toContain(KEY);
      expect(href.startsWith("https://")).toBe(true);
    }
  });

  it("opens Grok, Codex, ChatGPT, and Claude", () => {
    const ids = OPEN_AGENTS.map((agent) => agent.id);
    expect(ids).toEqual(["grok", "codex", "chatgpt", "claude"]);
    const prompt = openPrompt(URL);
    expect(OPEN_AGENTS[0].href(prompt)).toContain("grok.com");
    expect(OPEN_AGENTS[1].href(prompt)).toContain("chatgpt.com");
    expect(OPEN_AGENTS[1].href(prompt)).toContain("Codex");
    expect(OPEN_AGENTS[3].href(prompt)).toContain("claude.ai");
  });

  it("Grok Bot skill tells Grok how to send without putting keys in URLs", () => {
    expect(GROK_BOT_SKILL).toContain("POST https://www.copypaste.fyi/api/pastes");
    expect(GROK_BOT_SKILL).toContain("X-Paste-Key");
    expect(GROK_BOT_SKILL.toLowerCase()).toContain("never put tokens in");
    expect(grokBotHref("install")).toContain("grok.com");
  });

  it("strips #key= before handing the URL to Grok", () => {
    const leaked = `${URL}#key=${KEY}`;
    const prompt = openPrompt(leaked);
    expect(prompt).toContain(URL);
    expect(prompt).not.toContain(KEY);
    expect(prompt).not.toContain("key=");
    for (const agent of OPEN_AGENTS) {
      expect(decodeURIComponent(agent.href(prompt))).not.toContain(KEY);
    }
  });
});

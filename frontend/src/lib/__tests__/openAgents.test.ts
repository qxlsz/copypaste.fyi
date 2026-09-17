import { describe, expect, it } from "vitest";

import {
  GROK_BOT_SKILL,
  OPEN_AGENTS,
  agentHref,
  grokBotHref,
  openPrompt,
  pasteOpenUrl,
} from "../openAgents";

const URL = "https://www.copypaste.fyi/p/secret01";
const KEY = "super-secret-token-do-not-leak";

describe("openAgents", () => {
  it("never puts an encryption key in a third-party chat URL", () => {
    const prompt = openPrompt(URL);
    expect(prompt).toContain(URL);
    expect(prompt.toLowerCase()).not.toContain("key=");
    for (const agent of OPEN_AGENTS) {
      for (const ua of [
        "",
        "Mozilla/5.0 (Linux; Android 14)",
        "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0)",
      ]) {
        const href = agentHref(agent, prompt, ua);
        expect(href).not.toContain(KEY);
        expect(decodeURIComponent(href)).not.toContain(KEY);
      }
    }
  });

  it("opens Grok, Codex, ChatGPT, and Claude", () => {
    const ids = OPEN_AGENTS.map((agent) => agent.id);
    expect(ids).toEqual(["grok", "codex", "chatgpt", "claude"]);
    const prompt = openPrompt(URL);
    expect(agentHref(OPEN_AGENTS[0], prompt)).toContain("grok.com");
    expect(agentHref(OPEN_AGENTS[1], prompt)).toContain("chatgpt.com");
    expect(agentHref(OPEN_AGENTS[1], prompt)).toContain("Codex");
    expect(agentHref(OPEN_AGENTS[3], prompt)).toContain("claude.ai");
  });

  it("uses Android intents that fall back to HTTPS", () => {
    const prompt = openPrompt(URL);
    const grok = agentHref(OPEN_AGENTS[0], prompt, "Mozilla/5.0 (Linux; Android 14; Pixel)");
    expect(grok.startsWith("intent://grok.com")).toBe(true);
    expect(grok).toContain("package=ai.x.grok");
    expect(grok).toContain("S.browser_fallback_url=");
    const chatgpt = agentHref(OPEN_AGENTS[2], prompt, "Mozilla/5.0 (Linux; Android 14)");
    expect(chatgpt).toContain("package=com.openai.chatgpt");
    const claude = agentHref(OPEN_AGENTS[3], prompt, "Mozilla/5.0 (Linux; Android 14)");
    expect(claude).toContain("package=com.anthropic.claude");
  });

  it("builds a same-origin ?open= handoff without the key", () => {
    const handoff = pasteOpenUrl(`${URL}#key=${KEY}`, "grok");
    expect(handoff).toBe("https://www.copypaste.fyi/p/secret01?open=grok");
    expect(handoff).not.toContain(KEY);
  });

  it("Grok Bot skill tells Grok how to send without putting keys in URLs", () => {
    expect(GROK_BOT_SKILL).toContain("POST https://www.copypaste.fyi/api/pastes");
    expect(GROK_BOT_SKILL).toContain("X-Paste-Key");
    expect(GROK_BOT_SKILL.toLowerCase()).toContain("never put tokens in");
    expect(grokBotHref("install")).toContain("grok.com");
    expect(grokBotHref("install", "Mozilla/5.0 (Linux; Android 14)")).toContain(
      "package=ai.x.grok",
    );
  });

  it("strips #key= before handing the URL to Grok", () => {
    const leaked = `${URL}#key=${KEY}`;
    const prompt = openPrompt(leaked);
    expect(prompt).toContain(URL);
    expect(prompt).not.toContain(KEY);
    expect(prompt).not.toContain("key=");
    for (const agent of OPEN_AGENTS) {
      expect(decodeURIComponent(agentHref(agent, prompt))).not.toContain(KEY);
    }
  });
});

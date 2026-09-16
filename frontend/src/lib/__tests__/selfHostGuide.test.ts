import { describe, expect, it } from "vitest";

import { hostRecipe } from "../selfHostGuide";

describe("hostRecipe", () => {
  it("sends Grok VMs to agent-setup, not brew", () => {
    const recipe = hostRecipe("local", "grok");
    expect(recipe.follow).toMatch(/Grok/i);
    expect(recipe.commands).toMatch(/agent-setup\.sh --serve/);
    expect(recipe.commands).not.toMatch(/brew install/);
  });

  it("sends Cursor agents to the same VM script", () => {
    const recipe = hostRecipe("local", "cursor");
    expect(recipe.follow).toMatch(/Cursor/i);
    expect(recipe.commands).toMatch(/agent-setup\.sh/);
  });

  it("sends AWS to agent-setup and can pin Upstash Redis", () => {
    const recipe = hostRecipe("local", "aws", "redis");
    expect(recipe.follow).toMatch(/AWS/i);
    expect(recipe.commands).toMatch(/agent-setup\.sh/);
    expect(recipe.commands).toMatch(/COPYPASTE_PERSISTENCE_BACKEND=redis/);
    expect(recipe.commands).not.toMatch(/COPYPASTE_FORCE_MEMORY=true/);
  });

  it("sends connector goal to /mcp for Grok", () => {
    const recipe = hostRecipe("connector", "apple", "redis");
    expect(recipe.commands).toMatch(/\/mcp/);
    expect(recipe.commands).toMatch(/grok.com\/connectors/);
    expect(recipe.commands).toMatch(/--agent/);
  });
});

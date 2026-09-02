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

  it("keeps Apple on brew", () => {
    expect(hostRecipe("local", "apple").commands).toMatch(/brew install/);
  });
});

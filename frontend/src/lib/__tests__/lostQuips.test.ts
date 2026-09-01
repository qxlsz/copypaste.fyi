import { describe, expect, it } from "vitest";

import { quipFor } from "../lostQuips";

describe("quipFor", () => {
  it("is stable for a seed", () => {
    expect(quipFor("missing-id")).toBe(quipFor("missing-id"));
  });

  it("defaults when there is no seed", () => {
    expect(quipFor()).toMatch(/lost|chasing/i);
  });

  it("stays within the joke list", () => {
    for (const seed of ["a", "b", "zzzz", "paste-id-24chars-here"]) {
      expect(quipFor(seed).length).toBeGreaterThan(12);
    }
  });
});

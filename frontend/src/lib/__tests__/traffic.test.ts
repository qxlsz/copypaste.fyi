import { describe, expect, it } from "vitest";

import { pageBucket } from "../traffic";

describe("pageBucket", () => {
  it("never sends a paste id", () => {
    expect(pageBucket("/p/9LIhAn9e5WLd7Mo0n01LsxgK")).toBe("/p/");
    expect(pageBucket("/")).toBe("/");
    expect(pageBucket("/about")).toBe("/about");
  });
});

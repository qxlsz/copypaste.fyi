import { describe, expect, it } from "vitest";

import { incomingShareText } from "../shareIncoming";

describe("incomingShareText", () => {
  it("joins title, text, and url from a share-target query", () => {
    expect(incomingShareText("?title=note&text=hello&url=https://example.com")).toBe(
      "note\nhello\nhttps://example.com",
    );
  });

  it("dedupes identical fields", () => {
    expect(incomingShareText("text=same&title=same")).toBe("same");
  });
});

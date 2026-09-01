import { describe, expect, it } from "vitest";

import { pasteIdFromShareUrl } from "../shareImage";

describe("pasteIdFromShareUrl", () => {
  it("reads the paste id from a share URL", () => {
    expect(pasteIdFromShareUrl("https://www.copypaste.fyi/p/teHQyof5Ku")).toBe("teHQyof5Ku");
  });

  it("falls back when the URL is not a paste", () => {
    expect(pasteIdFromShareUrl("not a url")).toBe("paste");
  });
});

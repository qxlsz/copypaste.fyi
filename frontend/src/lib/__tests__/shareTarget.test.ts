import { describe, expect, it } from "vitest";

import { MAX_PASTE_BYTES } from "../composer";
import { shareTargetText, stripShareTargetSearch } from "../shareTarget";

describe("shareTarget", () => {
  it("returns null when no share fields exist", () => {
    expect(shareTargetText("")).toBeNull();
    expect(shareTargetText("?foo=bar")).toBeNull();
  });

  it("joins title, text, and url without duplicating the same string", () => {
    expect(shareTargetText("?text=hello")).toBe("hello");
    expect(shareTargetText("?title=Note&text=hello&url=https://example.com")).toBe(
      "Note\n\nhello\n\nhttps://example.com",
    );
    expect(shareTargetText("?title=same&text=same")).toBe("same");
  });

  it("strips share fields so the URL does not keep the payload", () => {
    expect(stripShareTargetSearch("?text=secret&theme=dark")).toBe("?theme=dark");
    expect(stripShareTargetSearch("?text=secret")).toBe("");
  });

  it("clips oversized payloads to the public byte cap", () => {
    const huge = "a".repeat(MAX_PASTE_BYTES + 40);
    const seeded = shareTargetText(`?text=${huge}`);
    expect(seeded).not.toBeNull();
    expect(new TextEncoder().encode(seeded ?? "").byteLength).toBe(MAX_PASTE_BYTES);
  });
});

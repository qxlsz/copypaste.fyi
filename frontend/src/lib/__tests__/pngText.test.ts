import { describe, expect, it } from "vitest";

import { injectPngText, readPngText } from "../pngText";

/** 1×1 transparent PNG. */
const TINY_PNG = Uint8Array.from(
  atob(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  ),
  (char) => char.charCodeAt(0),
);

describe("pngText", () => {
  it("round-trips a URL in a tEXt chunk before IEND", () => {
    const url = "https://www.copypaste.fyi/p/abc123";
    const next = injectPngText(TINY_PNG, "URL", url);
    expect(readPngText(next, "URL")).toBe(url);
    expect(readPngText(next, "Software")).toBeNull();
    expect(next[0]).toBe(0x89);
    expect(new TextDecoder("latin1").decode(next.slice(-8, -4))).toBe("IEND");
  });

  it("rejects a non-PNG", () => {
    expect(() => injectPngText(new Uint8Array([0, 1, 2]), "URL", "x")).toThrow(/Not a PNG/);
  });

  it("stacks Software and URL chunks", () => {
    const once = injectPngText(TINY_PNG, "Software", "copypaste.fyi");
    const twice = injectPngText(once, "URL", "https://www.copypaste.fyi/p/x");
    expect(readPngText(twice, "Software")).toBe("copypaste.fyi");
    expect(readPngText(twice, "URL")).toBe("https://www.copypaste.fyi/p/x");
  });

  it("rejects an empty keyword", () => {
    expect(() => injectPngText(TINY_PNG, "", "x")).toThrow(/keyword/);
  });
});

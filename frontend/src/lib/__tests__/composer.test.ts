import { describe, expect, it } from "vitest";

import { MAX_PASTE_BYTES, composerStats, sniffFormat } from "../composer";

describe("composer", () => {
  it("counts chars, lines, and bytes", () => {
    expect(composerStats("")).toEqual({ chars: 0, lines: 0, bytes: 0 });
    expect(composerStats("hi\nthere")).toEqual({ chars: 8, lines: 2, bytes: 8 });
    expect(composerStats("é").bytes).toBe(2);
  });

  it("sniffs a format from the filename", () => {
    expect(sniffFormat("notes.md")).toBe("markdown");
    expect(sniffFormat("app.rs")).toBe("rust");
    expect(sniffFormat("/tmp/foo.JSON")).toBe("json");
    expect(sniffFormat("mystery.bin")).toBe("code");
    expect(sniffFormat("Makefile")).toBeNull();
  });

  it("keeps the public 1 MiB cap", () => {
    expect(MAX_PASTE_BYTES).toBe(1048576);
  });
});

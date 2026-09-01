import { describe, expect, it } from "vitest";

import { whisperNote } from "../whisper";

describe("whisperNote", () => {
  it("carries the URL and refuses to promise a listing", () => {
    const note = whisperNote("https://www.copypaste.fyi/p/abc");
    expect(note).toContain("https://www.copypaste.fyi/p/abc");
    expect(note.toLowerCase()).toContain("not listed");
    expect(note.toLowerCase()).toContain("lost");
  });

  it("does not wrap the URL in a human-looking story that hides it", () => {
    const url = "https://www.copypaste.fyi/p/teHQyof5Ku";
    expect(whisperNote(url).startsWith(url)).toBe(true);
  });
});

import { describe, expect, it } from "vitest";

import { sniffFormatFromText } from "../sniffFormat";

describe("sniffFormatFromText", () => {
  it("stays quiet on short or empty text", () => {
    expect(sniffFormatFromText("")).toBeNull();
    expect(sniffFormatFromText("hi")).toBeNull();
  });

  it("picks json, rust, python, and markdown", () => {
    expect(sniffFormatFromText('{"ok": true, "n": 1}')).toBe("json");
    expect(sniffFormatFromText('fn main() {\n  println!("hi");\n}')).toBe("rust");
    expect(sniffFormatFromText("def greet(name):\n    return name")).toBe("python");
    expect(sniffFormatFromText("# Title\n\nA [link](https://copypaste.fyi).")).toBe("markdown");
  });

  it("reads a shebang and a fenced block", () => {
    expect(sniffFormatFromText("#!/usr/bin/env bash\necho hi")).toBe("bash");
    expect(sniffFormatFromText("```ts\nconst x = 1;\n```")).toBe("typescript");
  });
});

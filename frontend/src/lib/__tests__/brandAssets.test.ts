import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const publicDir = join(dirname(fileURLToPath(import.meta.url)), "../../../public");

describe("brand mark assets", () => {
  it.each(["copypaste.svg", "copypaste-light.svg", "copypaste-dark.svg"])(
    "%s is the two-sheet mark",
    (name) => {
      const svg = readFileSync(join(publicDir, name), "utf8");
      expect(svg).toContain("<svg");
      expect(svg).toContain('width="16.75"');
      expect(svg.match(/<rect/g)?.length).toBeGreaterThanOrEqual(2);
    },
  );

  it("light and dark marks use opposite ink", () => {
    const light = readFileSync(join(publicDir, "copypaste-light.svg"), "utf8");
    const dark = readFileSync(join(publicDir, "copypaste-dark.svg"), "utf8");
    expect(light).toContain("#1c1b18");
    expect(dark).toContain("#ece8df");
    expect(light).not.toContain("#ece8df");
  });
});

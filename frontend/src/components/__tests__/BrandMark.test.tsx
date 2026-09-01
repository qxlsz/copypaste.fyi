import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";

import { BrandMark } from "../BrandMark";

describe("BrandMark", () => {
  it("uses currentColor so the same mark works in light and dark", () => {
    const { container } = render(<BrandMark />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg?.getAttribute("viewBox")).toBe("0 0 32 32");
    const rects = container.querySelectorAll("rect");
    expect(rects).toHaveLength(2);
    expect(rects[0]?.getAttribute("stroke")).toBe("currentColor");
    expect(rects[1]?.getAttribute("fill")).toBe("currentColor");
  });
});

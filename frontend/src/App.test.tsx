import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import App from "./App";

describe("App", () => {
  it("renders without crashing", () => {
    expect(() => render(<App />)).not.toThrow();
  });

  it("renders the header branding", () => {
    render(<App />);
    expect(screen.getByText("copypaste")).toBeInTheDocument();
  });
});

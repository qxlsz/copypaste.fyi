import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import "@testing-library/jest-dom/vitest";

import { AboutPage } from "./About";

describe("AboutPage", () => {
  it("shows self-host commands", () => {
    render(
      <MemoryRouter>
        <AboutPage />
      </MemoryRouter>,
    );
    expect(screen.getByRole("heading", { name: /run your own/i })).toBeInTheDocument();
    expect(screen.getByText(/copypaste serve/i)).toBeInTheDocument();
    expect(screen.getByText(/127\.0\.0\.1:8000/)).toBeInTheDocument();
  });
});

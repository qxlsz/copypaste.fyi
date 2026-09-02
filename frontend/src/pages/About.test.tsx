import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import "@testing-library/jest-dom/vitest";

import { AboutPage } from "./About";

describe("AboutPage", () => {
  it("picks Grok VM instructions by default", () => {
    render(
      <MemoryRouter>
        <AboutPage />
      </MemoryRouter>,
    );
    expect(screen.getByRole("heading", { name: /run your own/i })).toBeInTheDocument();
    expect(screen.getByText(/follow this:/i)).toBeInTheDocument();
    expect(screen.getByText(/agent-setup\.sh --serve/)).toBeInTheDocument();
  });

  it("switches to Apple brew when asked", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <AboutPage />
      </MemoryRouter>,
    );
    await user.click(screen.getByRole("button", { name: "Apple" }));
    expect(screen.getByText(/brew install qxlsz\/copypaste\/copypaste/)).toBeInTheDocument();
  });
});

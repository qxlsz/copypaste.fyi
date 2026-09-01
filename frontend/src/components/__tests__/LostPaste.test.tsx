import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import "@testing-library/jest-dom/vitest";

import { LostPaste } from "../LostPaste";

describe("LostPaste", () => {
  it("does not say whether the paste expired or never existed", () => {
    render(
      <MemoryRouter>
        <LostPaste seed="missing-id" />
      </MemoryRouter>,
    );
    expect(screen.getByRole("heading").textContent?.length).toBeGreaterThan(8);
    expect(screen.getByText(/hunting a hidden secret/i)).toBeInTheDocument();
    expect(screen.queryByText(/paste expired/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/not found/i)).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: /new paste/i })).toHaveAttribute("href", "/");
  });
});

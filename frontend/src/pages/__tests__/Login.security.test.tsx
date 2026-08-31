import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import "@testing-library/jest-dom/vitest";

import { LoginPage } from "../Login";

describe("Login private-key handling", () => {
  it("disables browser text services and explains memory-only storage", () => {
    render(
      <MemoryRouter>
        <LoginPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByLabelText("Use existing private key"));
    const privateKey = screen.getByLabelText("private key (base64)");

    expect(privateKey).toHaveAttribute("autocomplete", "off");
    expect(privateKey).toHaveAttribute("autocapitalize", "none");
    expect(privateKey).toHaveAttribute("autocorrect", "off");
    expect(privateKey).toHaveAttribute("spellcheck", "false");
    expect(
      screen.getByText(/kept only in this tab's memory and cleared on reload/i),
    ).toBeInTheDocument();
  });
});

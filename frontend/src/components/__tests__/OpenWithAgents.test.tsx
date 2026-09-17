import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import "@testing-library/jest-dom/vitest";

import { OpenWithAgents } from "../OpenWithAgents";

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

describe("OpenWithAgents", () => {
  it("links out to Grok and Codex without the paste key", () => {
    render(<OpenWithAgents url="https://www.copypaste.fyi/p/abc#key=dont-leak" />);
    const grok = screen.getByRole("link", { name: "Grok" });
    const codex = screen.getByRole("link", { name: "Codex" });
    expect(grok).toHaveAttribute("href", expect.stringContaining("grok.com"));
    expect(codex).toHaveAttribute("href", expect.stringContaining("chatgpt.com"));
    expect(decodeURIComponent(grok.getAttribute("href") ?? "")).not.toContain("dont-leak");
    expect(screen.getByRole("button", { name: "Add to Grok" })).toBeInTheDocument();
    const gmail = screen.getByRole("link", { name: "Share with Gmail" });
    expect(gmail).toHaveAttribute("href", expect.stringContaining("mail.google.com"));
    expect(decodeURIComponent(gmail.getAttribute("href") ?? "")).not.toContain("dont-leak");
    const whatsapp = screen.getByRole("link", { name: "Share with WhatsApp" });
    expect(whatsapp).toHaveAttribute("href", expect.stringContaining("wa.me"));
    expect(decodeURIComponent(whatsapp.getAttribute("href") ?? "")).not.toContain("dont-leak");
    expect(screen.getByRole("button", { name: "Sign in with Google" })).toBeInTheDocument();
    expect(screen.queryByText("Open in Grok, Codex, ChatGPT…")).not.toBeInTheDocument();
    const chatgptSvg = screen.getByRole("link", { name: "ChatGPT" }).querySelector("svg");
    const codexSvg = screen.getByRole("link", { name: "Codex" }).querySelector("svg");
    expect(chatgptSvg?.innerHTML).not.toEqual(codexSvg?.innerHTML);
  });
});

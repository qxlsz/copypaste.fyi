import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom/vitest";

import { App } from "./App";
import { ThemeProvider } from "./theme/ThemeProvider";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
    mutations: { retry: false },
  },
});

const renderApp = () => {
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <ThemeProvider>
          <App />
        </ThemeProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
};

describe("App", () => {
  it("renders without crashing", () => {
    expect(() => renderApp()).not.toThrow();
  });

  it("renders the basic layout elements", () => {
    renderApp();

    // Header wordmark links home
    expect(
      screen.getAllByRole("link", { name: /copypaste\.fyi home/i }).length,
    ).toBeGreaterThan(0);
    // Composer primary action
    expect(
      screen.getAllByRole("button", { name: "Get link" }).length,
    ).toBeGreaterThan(0);
  });
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";
import { ErrorBoundary } from "../ErrorBoundary";

const ThrowOnce = ({ shouldThrow }: { shouldThrow: boolean }) => {
  if (shouldThrow) {
    throw new Error("Render error from test");
  }
  return <div>rendered successfully</div>;
};

describe("ErrorBoundary", () => {
  beforeEach(() => {
    // Suppress React's error boundary console output during tests
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  it("renders children when no error occurs", () => {
    render(
      <ErrorBoundary>
        <div>child content</div>
      </ErrorBoundary>,
    );
    expect(screen.getByText("child content")).toBeInTheDocument();
  });

  it("shows default fallback UI when child throws", () => {
    render(
      <ErrorBoundary>
        <ThrowOnce shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /try again/i })).toBeInTheDocument();
  });

  it("shows custom fallback when provided and child throws", () => {
    render(
      <ErrorBoundary fallback={<p>Custom error message</p>}>
        <ThrowOnce shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Custom error message")).toBeInTheDocument();
    expect(screen.queryByText("Something went wrong")).not.toBeInTheDocument();
  });

  it("resets error state when Try again is clicked", () => {
    const { rerender } = render(
      <ErrorBoundary>
        <ThrowOnce shouldThrow={true} />
      </ErrorBoundary>,
    );

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();

    // Update to non-throwing children first; React 19 replays the render on
    // reset, so props must be non-throwing before clicking "Try again".
    rerender(
      <ErrorBoundary>
        <ThrowOnce shouldThrow={false} />
      </ErrorBoundary>,
    );

    fireEvent.click(screen.getByRole("button", { name: /try again/i }));

    expect(screen.getByText("rendered successfully")).toBeInTheDocument();
  });
});

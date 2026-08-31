import { afterEach, describe, expect, it, vi } from "vitest";

import { STORAGE_KEY, getInitialTheme } from "../themeStorage";

const originalMatchMedia = window.matchMedia;
const originalLocalStorage = window.localStorage;

afterEach(() => {
  Object.defineProperty(window, "matchMedia", {
    value: originalMatchMedia,
    writable: true,
  });

  Object.defineProperty(window, "localStorage", {
    value: originalLocalStorage,
    writable: true,
  });
});

describe("getInitialTheme", () => {
  it("defaults to light when window is undefined", () => {
    Object.defineProperty(window, "matchMedia", {
      value: originalMatchMedia,
      writable: true,
    });
    Object.defineProperty(window, "localStorage", {
      value: originalLocalStorage,
      writable: true,
    });

    expect(getInitialTheme()).toBe("light");
  });

  it("uses stored theme when available", () => {
    const getItem = vi.fn((key: string) =>
      key === STORAGE_KEY ? "dark" : null,
    );
    Object.defineProperty(window, "localStorage", {
      value: { getItem },
      writable: true,
    });
    Object.defineProperty(window, "matchMedia", {
      value: vi.fn(() => ({ matches: false })),
      writable: true,
    });

    expect(getInitialTheme()).toBe("dark");
  });

  it("falls back to prefers-color-scheme when no storage value", () => {
    Object.defineProperty(window, "localStorage", {
      value: { getItem: () => null },
      writable: true,
    });
    Object.defineProperty(window, "matchMedia", {
      value: vi.fn(() => ({ matches: true })),
      writable: true,
    });

    expect(getInitialTheme()).toBe("dark");
  });
});

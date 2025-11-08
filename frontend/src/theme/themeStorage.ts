export const STORAGE_KEY = "copypaste.theme";

export const getInitialTheme = (): "light" | "dark" => {
  if (typeof window === "undefined") {
    return "light";
  }
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (stored === "light" || stored === "dark") {
    return stored;
  }
  const prefersDark = window.matchMedia?.("(prefers-color-scheme: dark)");
  return prefersDark?.matches ? "dark" : "light";
};

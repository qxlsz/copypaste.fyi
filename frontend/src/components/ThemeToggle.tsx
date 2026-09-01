import clsx from "clsx";

import { useTheme } from "../theme/ThemeContext";

export const ThemeToggle = () => {
  const { theme, toggleTheme } = useTheme();
  const toLight = theme === "dark";
  const label = toLight ? "Switch to light mode" : "Switch to dark mode";

  return (
    <button
      type="button"
      onClick={toggleTheme}
      className={clsx(
        "inline-flex size-11 appearance-none items-center justify-center rounded-lg bg-transparent",
        "text-muted-foreground transition hover:bg-muted hover:text-text",
        "focus-visible:outline-none sm:size-10",
      )}
      aria-label={label}
      title={label}
    >
      {toLight ? (
        <svg viewBox="0 0 24 24" className="size-4" aria-hidden="true">
          <circle cx="12" cy="12" r="4" fill="currentColor" />
          <path
            stroke="currentColor"
            strokeWidth="1.75"
            strokeLinecap="round"
            d="M12 3v1.5M12 19.5V21M3 12h1.5M19.5 12H21M5.64 5.64l1.06 1.06M17.3 17.3l1.06 1.06M5.64 18.36l1.06-1.06M17.3 6.7l1.06-1.06"
          />
        </svg>
      ) : (
        <svg viewBox="0 0 24 24" className="size-4" aria-hidden="true">
          <path
            fill="currentColor"
            d="M16.4 13.2A6.4 6.4 0 0 1 10.8 7.6 6.2 6.2 0 1 0 16.4 13.2Z"
          />
        </svg>
      )}
    </button>
  );
};

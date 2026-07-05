import { Suspense, useMemo, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { BarChart2, Command, Plus, SquareTerminal } from "lucide-react";

import { ThemeToggle } from "./ThemeToggle";
import { CommandPalette } from "./CommandPalette";
import { useHotkeys } from "../hooks/useHotkeys";
import { useAuth } from "../stores/auth";
import { useTheme } from "../theme/ThemeContext";

const iconButtonClasses =
  "inline-flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface";

export const Layout = () => {
  const navigate = useNavigate();
  const [isPaletteOpen, setPaletteOpen] = useState(false);
  const location = useLocation();
  const { user, logout } = useAuth();
  const { toggleTheme } = useTheme();

  const isWidePage =
    location.pathname === "/" || location.pathname.startsWith("/p/");

  const commandActions = useMemo(
    () => [
      {
        id: "create-paste",
        label: "Create new paste",
        description:
          "Jump straight to the composer with default retention and encryption.",
        shortcut: "⌘N",
        group: "Primary",
        handler: () => navigate("/"),
      },
      {
        id: "about",
        label: "About & architecture",
        group: "Navigation",
        handler: () => navigate("/about"),
      },
      {
        id: "stats",
        label: "Service statistics",
        group: "Navigation",
        handler: () => navigate("/stats"),
      },
      {
        id: "toggle-theme",
        label: "Toggle dark / light mode",
        group: "Preferences",
        handler: () => toggleTheme(),
      },
    ],
    [navigate, toggleTheme],
  );

  useHotkeys({ shortcut: "meta+n", handler: () => navigate("/") });
  useHotkeys({ shortcut: "ctrl+n", handler: () => navigate("/") });

  return (
    <div className="flex min-h-screen flex-col bg-background text-text transition-colors duration-150">
      <CommandPalette
        actions={commandActions}
        isOpen={isPaletteOpen}
        onOpenChange={setPaletteOpen}
      />
      <header className="border-b border-border bg-surface">
        <div className="mx-auto flex h-[52px] max-w-6xl items-center justify-between px-4 sm:px-6">
          <NavLink
            to="/"
            className="group inline-flex items-baseline gap-px rounded-md font-mono text-sm font-medium lowercase text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface"
            aria-label="copypaste.fyi home"
          >
            copypaste
            <span
              aria-hidden="true"
              className="ml-0.5 inline-block h-[0.9em] w-[0.5em] translate-y-px bg-accent transition-opacity group-hover:opacity-60"
            />
          </NavLink>
          <nav className="flex items-center gap-1" aria-label="Primary">
            <NavLink
              to="/about"
              className="mr-1 hidden rounded-md px-2 py-1 text-xs text-muted-foreground transition hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface sm:inline-block"
            >
              about
            </NavLink>
            <button
              onClick={() => navigate("/")}
              className={iconButtonClasses}
              aria-label="Create new paste"
              title="New paste (⌘N)"
            >
              <Plus className="h-4 w-4" aria-hidden="true" />
            </button>
            <button
              onClick={() => navigate("/stats")}
              className={iconButtonClasses}
              aria-label="Service statistics"
              title="Stats"
            >
              <BarChart2 className="h-4 w-4" aria-hidden="true" />
            </button>
            {user && (
              <button
                onClick={() => navigate("/dashboard")}
                className={iconButtonClasses}
                aria-label="Go to dashboard"
                title="Dashboard"
              >
                <SquareTerminal className="h-4 w-4" aria-hidden="true" />
              </button>
            )}
            <button
              onClick={() => setPaletteOpen(true)}
              className={iconButtonClasses}
              aria-label="Open command menu"
              title="Command Menu (⌘K)"
            >
              <Command className="h-4 w-4" aria-hidden="true" />
            </button>
            <ThemeToggle />
            <span
              className="ml-1 hidden select-none rounded border border-border px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground md:inline-block"
              aria-hidden="true"
            >
              ⌘K
            </span>
            <button
              onClick={() => {
                if (user) {
                  logout();
                  navigate("/");
                } else {
                  navigate("/login");
                }
              }}
              className="ml-2 inline-flex h-8 items-center rounded-md border border-border px-3 text-xs font-medium text-text transition hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface"
            >
              {user ? "Logout" : "Login"}
            </button>
          </nav>
        </div>
      </header>
      <main
        className={`mx-auto w-full flex-1 px-4 py-8 sm:px-6 ${
          isWidePage ? "max-w-6xl" : "max-w-5xl"
        }`}
      >
        <Suspense
          fallback={
            <div
              className="flex min-h-[40vh] items-center justify-center"
              role="status"
              aria-label="Loading page"
            >
              <span className="h-5 w-5 animate-spin rounded-full border-2 border-border border-t-accent" />
            </div>
          }
        >
          <Outlet />
        </Suspense>
      </main>
      <footer className="border-t border-border">
        <div className="mx-auto flex max-w-6xl items-center gap-2 px-4 py-4 text-xs text-muted-foreground sm:px-6">
          <span>open source</span>
          <span aria-hidden="true">·</span>
          <NavLink to="/about" className="transition hover:text-text">
            about
          </NavLink>
          <span aria-hidden="true">·</span>
          <a
            href="https://github.com/qxlsz/copypaste.fyi"
            target="_blank"
            rel="noopener noreferrer"
            className="transition hover:text-text"
          >
            github
          </a>
          <span className="ml-auto font-mono">copypaste.fyi</span>
        </div>
      </footer>
    </div>
  );
};

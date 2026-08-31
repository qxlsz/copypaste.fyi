import { Suspense, useMemo, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { BarChart2, Command, Plus, SquareTerminal } from "lucide-react";

import { ThemeToggle } from "./ThemeToggle";
import { CommandPalette } from "./CommandPalette";
import { useHotkeys } from "../hooks/useHotkeys";
import { useAuth } from "../stores/auth";
import { useTheme } from "../theme/ThemeContext";

const iconButtonClasses =
  "inline-flex size-11 items-center justify-center text-muted-foreground transition hover:text-text focus-visible:outline-none sm:size-8";

export const Layout = () => {
  const navigate = useNavigate();
  const [isPaletteOpen, setPaletteOpen] = useState(false);
  const location = useLocation();
  const { user, logout } = useAuth();
  const { toggleTheme } = useTheme();

  const isEditorPage =
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

  const wordmark = (
    <NavLink
      to="/"
      className="inline-flex min-h-11 items-center gap-2 font-mono text-sm font-medium tracking-tight text-text focus-visible:outline-none"
      aria-label="copypaste.fyi home"
    >
      <span className="relative block size-4" aria-hidden="true">
        <span className="absolute left-0 top-0 size-2.5 border border-current" />
        <span className="absolute bottom-0 right-0 size-2.5 bg-current" />
      </span>
      <span className="sm:sr-only">copypaste</span>
    </NavLink>
  );

  const navButtons = (
    <>
      <button
        type="button"
        onClick={() => navigate("/")}
        className={iconButtonClasses}
        aria-label="Create new paste"
        title="New paste (⌘N)"
      >
        <Plus className="h-4 w-4" aria-hidden="true" />
      </button>
      <button
        type="button"
        onClick={() => navigate("/stats")}
        className={iconButtonClasses}
        aria-label="Service statistics"
        title="Stats"
      >
        <BarChart2 className="h-4 w-4" aria-hidden="true" />
      </button>
      {user && (
        <button
          type="button"
          onClick={() => navigate("/dashboard")}
          className={iconButtonClasses}
          aria-label="Go to dashboard"
          title="Dashboard"
        >
          <SquareTerminal className="h-4 w-4" aria-hidden="true" />
        </button>
      )}
      <button
        type="button"
        onClick={() => setPaletteOpen(true)}
        className={iconButtonClasses}
        aria-label="Open command menu"
        title="Command Menu (⌘K)"
      >
        <Command className="h-4 w-4" aria-hidden="true" />
      </button>
      <ThemeToggle />
    </>
  );

  return (
    <div className="flex h-dvh flex-col overflow-hidden bg-background text-text sm:flex-row">
      <CommandPalette
        actions={commandActions}
        isOpen={isPaletteOpen}
        onOpenChange={setPaletteOpen}
      />

      <header className="shrink-0 border-b border-border bg-gutter pt-[env(safe-area-inset-top)] sm:hidden">
        <div className="flex h-12 items-center gap-1 px-3">
          {wordmark}
          <nav className="ml-auto flex items-center" aria-label="Primary">
            {navButtons}
            <button
              type="button"
              onClick={() => {
                void (async () => {
                  if (user) {
                    await logout();
                    navigate("/");
                  } else {
                    navigate("/login");
                  }
                })();
              }}
              className="ml-1 inline-flex h-11 items-center px-2 font-mono text-xs font-medium text-text"
            >
              {user ? "Logout" : "Login"}
            </button>
          </nav>
        </div>
      </header>

      <nav
        aria-label="Primary"
        className="hidden w-14 shrink-0 flex-col items-center border-r border-border bg-gutter pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] sm:flex"
      >
        <NavLink
          to="/"
          aria-label="copypaste.fyi home"
          title="copypaste.fyi"
          className="inline-flex size-14 items-center justify-center text-text"
        >
          <span className="relative block size-4" aria-hidden="true">
            <span className="absolute left-0 top-0 size-2.5 border border-current" />
            <span className="absolute bottom-0 right-0 size-2.5 bg-current" />
          </span>
        </NavLink>
        {navButtons}
        <div className="mt-auto flex flex-col items-center gap-1 pb-2">
          <NavLink
            to="/about"
            className="inline-flex size-8 items-center justify-center font-mono text-[10px] text-muted-foreground transition hover:text-text"
          >
            about
          </NavLink>
          <button
            type="button"
            onClick={() => {
              void (async () => {
                if (user) {
                  await logout();
                  navigate("/");
                } else {
                  navigate("/login");
                }
              })();
            }}
            className="inline-flex h-8 items-center px-2 font-mono text-[10px] text-muted-foreground transition hover:text-text"
          >
            {user ? "out" : "in"}
          </button>
        </div>
      </nav>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        <main
          className={
            isEditorPage
              ? "flex min-h-0 flex-1 flex-col overflow-hidden"
              : "min-h-0 flex-1 overflow-auto px-4 py-6 sm:px-6"
          }
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
        {!isEditorPage && (
          <footer className="shrink-0 border-t border-border">
            <div className="flex items-center gap-2 px-4 py-3 text-xs text-muted-foreground sm:px-6">
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
        )}
      </div>
    </div>
  );
};

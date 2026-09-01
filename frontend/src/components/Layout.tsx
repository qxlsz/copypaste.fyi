import { Suspense, useMemo, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { BarChart2, Command, Plus } from "lucide-react";

import { CommandPalette } from "./CommandPalette";
import { BrandMark } from "./BrandMark";
import { ThemeToggle } from "./ThemeToggle";
import { useHotkeys } from "../hooks/useHotkeys";
import { useKeyboardInset } from "../hooks/useKeyboardInset";
import { useTheme } from "../theme/ThemeContext";

const iconButtonClasses =
  "inline-flex size-11 appearance-none items-center justify-center rounded-lg bg-transparent text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none sm:size-10";

export const Layout = () => {
  const navigate = useNavigate();
  const [isPaletteOpen, setPaletteOpen] = useState(false);
  const location = useLocation();
  const { toggleTheme } = useTheme();
  useKeyboardInset();

  const isEditorPage = location.pathname === "/" || location.pathname.startsWith("/p/");

  const commandActions = useMemo(
    () => [
      {
        id: "create-paste",
        label: "Create new paste",
        description: "Jump straight to the composer with default retention and encryption.",
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
    <div className="flex h-dvh flex-col overflow-hidden bg-background text-text">
      <CommandPalette
        actions={commandActions}
        isOpen={isPaletteOpen}
        onOpenChange={setPaletteOpen}
      />

      <header className="shrink-0 border-b border-border bg-background/80 pt-[env(safe-area-inset-top)] backdrop-blur-md">
        <div className="flex h-14 items-center gap-1 px-3 sm:px-4">
          <NavLink
            to="/"
            className="inline-flex size-11 items-center justify-center text-text focus-visible:outline-none"
            aria-label="copypaste.fyi home"
            title="copypaste.fyi"
          >
            <BrandMark />
          </NavLink>
          <nav className="ml-auto flex items-center gap-0.5" aria-label="Primary">
            {location.pathname !== "/" && (
              <button
                type="button"
                onClick={() => navigate("/")}
                className={`${iconButtonClasses} max-sm:hidden`}
                aria-label="Create new paste"
                title="New paste (⌘N)"
              >
                <Plus className="h-4 w-4" aria-hidden="true" />
              </button>
            )}
            <button
              type="button"
              onClick={() => navigate("/stats")}
              className={`${iconButtonClasses} max-sm:hidden`}
              aria-label="Service statistics"
              title="Stats"
            >
              <BarChart2 className="h-4 w-4" aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={() => setPaletteOpen(true)}
              className={`${iconButtonClasses} max-sm:hidden`}
              aria-label="Open command menu"
              title="Command Menu (⌘K)"
            >
              <Command className="h-4 w-4" aria-hidden="true" />
            </button>
            <ThemeToggle />
          </nav>
        </div>
      </header>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        <main
          className={
            isEditorPage
              ? "flex min-h-0 flex-1 flex-col overflow-hidden"
              : "min-h-0 flex-1 overflow-auto px-5 py-10 sm:px-6 sm:py-14"
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

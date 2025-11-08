import { useMemo, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";

import { ThemeToggle } from "./ThemeToggle";
import { Button } from "./ui/Button";
import { CommandPalette } from "./CommandPalette";
import { FloatingAboutButton } from "./FloatingAboutButton";
import { useHotkeys } from "../hooks/useHotkeys";
import { useAuth } from "../stores/auth";

export const Layout = () => {
  const navigate = useNavigate();
  const [isPaletteOpen, setPaletteOpen] = useState(false);
  const location = useLocation();
  const showHero = location.pathname === "/";
  const { user, logout } = useAuth();

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
    ],
    [navigate],
  );

  useHotkeys({ shortcut: "meta+n", handler: () => navigate("/") });
  useHotkeys({ shortcut: "ctrl+n", handler: () => navigate("/") });

  return (
    <div className="min-h-screen bg-background text-slate-900 transition-colors duration-300 dark:text-slate-100">
      <CommandPalette
        actions={commandActions}
        isOpen={isPaletteOpen}
        onOpenChange={setPaletteOpen}
      />
      <header className="border-b border-white/40 bg-surface/90 shadow-[0_20px_45px_-32px_rgba(0,25,80,0.35)] backdrop-blur-sm transition-colors dark:border-slate-800/60 dark:bg-slate-900/70 dark:shadow-none">
        <div className="mx-auto flex max-w-6xl flex-col gap-3 px-6 py-4">
          <div className="flex flex-col items-start justify-between gap-2 md:flex-row md:items-center">
            <div className="flex items-center gap-2">
              <NavLink
                to="/"
                className="text-sm font-semibold uppercase tracking-[0.32em] text-slate-700 transition hover:text-primary dark:text-slate-200 dark:hover:text-primary"
              >
                copypaste.fyi
              </NavLink>
              <ThemeToggle />
            </div>
            <nav className="flex flex-wrap items-center gap-2 md:gap-3">
              <button
                onClick={() => navigate("/")}
                className="inline-flex h-9 w-9 items-center justify-center rounded-full border border-emerald-300 bg-emerald-50 text-emerald-700 font-medium shadow-sm transition hover:border-emerald-400 hover:bg-emerald-100 hover:text-emerald-800 focus:outline-none focus:ring focus:ring-emerald-500/30 dark:border-emerald-700 dark:bg-emerald-900/50 dark:text-emerald-300 dark:hover:border-emerald-600 dark:hover:bg-emerald-800 dark:hover:text-emerald-200"
                aria-label="Create new paste"
                title="New paste (⌘N)"
              >
                <svg
                  className="h-4 w-4"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 4v16m8-8H4"
                  />
                </svg>
              </button>
              <div className="flex-1" />
              {user && (
                <button
                  onClick={() => navigate("/dashboard")}
                  className="inline-flex h-9 w-9 items-center justify-center rounded-full border border-slate-300 bg-surface text-xs font-medium shadow-sm transition hover:border-primary hover:text-primary focus:outline-none focus:ring focus:ring-primary/30 dark:border-slate-700 dark:hover:border-accent"
                  aria-label="Go to dashboard"
                  title="Dashboard"
                >
                  <svg
                    className="h-4 w-4"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
                    />
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M8 5a2 2 0 012-2h4a2 2 0 012 2v2H8V5z"
                    />
                  </svg>
                </button>
              )}

              <button
                onClick={() => setPaletteOpen(true)}
                className="inline-flex h-9 w-9 items-center justify-center rounded-full border border-slate-300 bg-surface text-xs font-medium shadow-sm transition hover:border-primary hover:text-primary focus:outline-none focus:ring focus:ring-primary/30 dark:border-slate-700 dark:hover:border-accent"
                aria-label="Open command menu"
                title="Command Menu (⌘K)"
              >
                <svg
                  className="h-4 w-4"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
                  />
                </svg>
              </button>
              <Button
                variant={user ? "secondary" : "primary"}
                size="sm"
                onClick={() => {
                  if (user) {
                    logout();
                    navigate("/");
                  } else {
                    navigate("/login");
                  }
                }}
              >
                {user ? "Logout" : "Login"}
              </Button>
            </nav>
          </div>
          {showHero && (
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Secure paste — encrypt, time-limit, or burn after reading. Login
              to track your pastes. Your keys stay local.
            </p>
          )}
        </div>
      </header>
      <main className="mx-auto max-w-6xl px-6 py-10">
        <Outlet />
      </main>
      <FloatingAboutButton />
    </div>
  );
};

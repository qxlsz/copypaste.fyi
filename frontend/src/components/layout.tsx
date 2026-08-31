import { useMemo, useState, type ReactNode } from "react";
import { Link, useNavigate, useRouterState } from "@tanstack/react-router";
import { Clock3, Command, Info, Moon, Plus, Sun, Terminal } from "lucide-react";
import { BrandMark } from "@/components/brand-mark";
import { CommandPalette, type CommandAction } from "@/components/command-palette";
import { PrivacyJourney } from "@/components/privacy-journey";
import { useTheme } from "@/components/theme-provider";
import { useHotkeys } from "@/hooks/use-hotkeys";
import { listRecents } from "@/lib/recents";
import { cn } from "@/lib/utils";

const iconBtn =
  "inline-flex size-11 items-center justify-center text-muted-foreground transition-colors duration-150 hover:text-foreground";
const railBtn =
  "relative inline-flex size-11 items-center justify-center text-muted-foreground transition-colors duration-150 hover:text-foreground";

export function AppShell({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const { theme, toggleTheme } = useTheme();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const home = pathname === "/";
  const editor = home || pathname.startsWith("/p/") || pathname.startsWith("/raw/");

  const actions = useMemo<CommandAction[]>(() => {
    const recents = typeof window === "undefined" ? [] : listRecents().slice(0, 8);
    return [
      {
        id: "new",
        label: "Create new paste",
        group: "Primary",
        shortcut: "⌘N",
        handler: () => navigate({ to: "/" }),
      },
      {
        id: "recents",
        label: "This-device recents",
        group: "Navigation",
        handler: () => navigate({ to: "/recents" }),
      },
      {
        id: "about",
        label: "About & security",
        group: "Navigation",
        handler: () => navigate({ to: "/about" }),
      },
      {
        id: "install",
        label: "CLI / packages / agents",
        group: "Navigation",
        handler: () => navigate({ to: "/install" }),
      },
      {
        id: "theme",
        label: "Toggle dark / light mode",
        group: "Preferences",
        handler: toggleTheme,
      },
      ...recents.map((item) => ({
        id: `paste-${item.id}`,
        label: item.preview,
        group: "This device",
        handler: () => navigate({ to: "/p/$id", params: { id: item.id } }),
      })),
    ];
  }, [navigate, toggleTheme, paletteOpen]);

  useHotkeys({ shortcut: "mod+n", handler: () => navigate({ to: "/" }) });

  return (
    <div className="flex h-dvh flex-col overflow-hidden bg-background text-foreground sm:flex-row">
      <CommandPalette actions={actions} open={paletteOpen} onOpenChange={setPaletteOpen} />

      <header className="shrink-0 border-b border-border pt-[env(safe-area-inset-top)] sm:hidden">
        <div className="flex h-12 items-center gap-1 px-3">
          <Link
            to="/"
            aria-label="copypaste.fyi home"
            className="inline-flex min-h-11 items-center gap-2 pr-2 text-foreground"
          >
            <BrandMark />
            <span className="font-mono text-sm font-medium tracking-tight">copypaste</span>
          </Link>
          <div className="ml-auto flex items-center">
            {!home && (
              <button
                type="button"
                onClick={() => navigate({ to: "/" })}
                className={iconBtn}
                aria-label="New paste"
                title="New paste"
              >
                <Plus className="size-4" />
              </button>
            )}
            <button
              type="button"
              onClick={() => navigate({ to: "/recents" })}
              className={iconBtn}
              aria-label="This-device recents"
              title="Recents"
            >
              <Clock3 className="size-4" />
            </button>
            <button
              type="button"
              onClick={toggleTheme}
              className={iconBtn}
              aria-label="Toggle color theme"
              title="Toggle theme"
            >
              {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
            </button>
            <button
              type="button"
              onClick={() => setPaletteOpen(true)}
              className={iconBtn}
              aria-label="Open menu"
              title="Menu"
            >
              <Command className="size-4" />
            </button>
          </div>
        </div>
      </header>

      <nav
        aria-label="Primary"
        className="hidden w-14 shrink-0 flex-col items-center border-r border-border bg-gutter pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] sm:flex"
      >
        <Link
          to="/"
          aria-label="copypaste.fyi home"
          title="copypaste.fyi"
          className="inline-flex size-14 items-center justify-center text-foreground"
        >
          <BrandMark />
        </Link>
        <RailButton
          current={home}
          label="New paste"
          title="New paste (⌘N)"
          onClick={() => navigate({ to: "/" })}
        >
          <Plus className="size-4" />
        </RailButton>
        <RailButton
          current={pathname === "/recents"}
          label="This-device recents"
          title="Recents"
          onClick={() => navigate({ to: "/recents" })}
        >
          <Clock3 className="size-4" />
        </RailButton>
        <RailButton
          label="Open command menu"
          title="Command menu (⌘K)"
          onClick={() => setPaletteOpen(true)}
        >
          <Command className="size-4" />
        </RailButton>
        <RailButton label="Toggle color theme" title="Toggle theme" onClick={toggleTheme}>
          {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
        </RailButton>
        <div className="mt-auto flex flex-col items-center pb-1">
          <PrivacyJourney variant="rail" />
          <Link
            to="/install"
            aria-label="CLI and packages"
            title="CLI"
            className={cn(railBtn, pathname === "/install" && "text-foreground")}
          >
            <Terminal className="size-4" />
          </Link>
          <Link
            to="/about"
            aria-label="About and security"
            title="About"
            className={cn(railBtn, pathname === "/about" && "text-foreground")}
          >
            <Info className="size-4" />
          </Link>
        </div>
      </nav>

      <div
        className={cn(
          "flex min-h-0 min-w-0 flex-1 flex-col",
          editor ? "overflow-hidden" : "overflow-auto",
        )}
      >
        {editor ? (
          children
        ) : (
          <main className="mx-auto w-full max-w-3xl px-4 py-6 pb-24 sm:px-6 sm:py-8 sm:pb-12">
            {children}
          </main>
        )}
      </div>
    </div>
  );
}

function RailButton({
  current,
  label,
  title,
  onClick,
  children,
}: {
  current?: boolean;
  label: string;
  title: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      aria-current={current ? "page" : undefined}
      title={title}
      className={cn(
        railBtn,
        current &&
          "text-foreground after:absolute after:left-0 after:h-4 after:w-px after:bg-foreground",
      )}
    >
      {children}
    </button>
  );
}

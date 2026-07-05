import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { createPortal } from "react-dom";

import { useHotkeys } from "../hooks/useHotkeys.ts";
import type { CommandPaletteAction } from "../types/commandPalette";

interface CommandPaletteProps {
  actions: CommandPaletteAction[];
  isOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
}

interface CommandPaletteOverlayProps {
  isOpen: boolean;
  onClose: () => void;
  query: string;
  onQueryChange: (value: string) => void;
  actions: CommandPaletteAction[];
  onSelect: (action: CommandPaletteAction) => void;
}

const focusableSelector =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

const CommandPaletteOverlay = ({
  isOpen,
  onClose,
  query,
  onQueryChange,
  actions,
  onSelect,
}: CommandPaletteOverlayProps) => {
  const [mounted, setMounted] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    setMounted(true);
    return () => setMounted(false);
  }, []);

  // Focus management: remember the trigger, focus the input on open, and
  // restore focus to the trigger when the palette closes.
  useEffect(() => {
    if (isOpen) {
      previousFocusRef.current =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      inputRef.current?.focus();
      return () => {
        previousFocusRef.current?.focus();
      };
    }
  }, [isOpen]);

  // Focus trap: keep Tab / Shift+Tab cycling within the panel.
  const handleKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab") return;
    const panel = panelRef.current;
    if (!panel) return;
    const focusable = Array.from(
      panel.querySelectorAll<HTMLElement>(focusableSelector),
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (event.shiftKey) {
      if (active === first || !panel.contains(active)) {
        event.preventDefault();
        last.focus();
      }
    } else if (active === last || !panel.contains(active)) {
      event.preventDefault();
      first.focus();
    }
  };

  if (!isOpen || !mounted) return null;

  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-[15vh]"
      onClick={onClose}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        onClick={(event) => event.stopPropagation()}
        onKeyDown={handleKeyDown}
        className="w-full max-w-lg rounded-lg border border-border bg-surface"
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-2.5">
          <span className="text-xs uppercase tracking-wide text-muted-foreground">
            Command palette
          </span>
          <span className="rounded border border-border px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
            ⌘K
          </span>
        </div>
        <div className="border-b border-border px-4 py-2">
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="Type a command or search…"
            aria-label="Search commands"
            className="w-full border-0 bg-transparent p-1 font-mono text-sm text-text placeholder:text-muted-foreground focus:outline-none focus:ring-0"
          />
        </div>
        <div className="max-h-[50vh] space-y-3 overflow-y-auto p-2">
          {actions.length ? (
            Object.entries(
              actions.reduce<Record<string, CommandPaletteAction[]>>(
                (grouped, action) => {
                  const group = action.group ?? "General";
                  if (!grouped[group]) {
                    grouped[group] = [];
                  }
                  grouped[group].push(action);
                  return grouped;
                },
                {},
              ),
            ).map(([group, groupActions]) => (
              <div key={group}>
                <p className="px-2 pb-1 pt-2 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                  {group}
                </p>
                <div className="grid gap-0.5">
                  {groupActions.map((action) => (
                    <button
                      key={action.id}
                      type="button"
                      onClick={() => onSelect(action)}
                      className="flex items-center justify-between rounded-md px-2 py-1.5 text-left text-sm text-text transition hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent"
                    >
                      <span>
                        {action.label}
                        {action.description ? (
                          <span className="block text-xs text-muted-foreground">
                            {action.description}
                          </span>
                        ) : null}
                      </span>
                      {action.shortcut ? (
                        <span className="ml-3 flex-shrink-0 font-mono text-[10px] text-muted-foreground">
                          {action.shortcut}
                        </span>
                      ) : null}
                    </button>
                  ))}
                </div>
              </div>
            ))
          ) : (
            <div className="px-4 py-10 text-center text-sm text-muted-foreground">
              No commands found{query ? ` for "${query}"` : ""}.
            </div>
          )}
        </div>
        <div className="flex items-center justify-between border-t border-border px-4 py-2 text-[10px] text-muted-foreground">
          <span className="font-mono">esc to close</span>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md px-2 py-1 text-xs text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            Close
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
};

export const CommandPalette = ({
  actions,
  isOpen: controlledOpen,
  onOpenChange,
}: CommandPaletteProps) => {
  const [internalOpen, setInternalOpen] = useState(false);
  const [query, setQuery] = useState("");

  const isControlled = typeof controlledOpen === "boolean";
  const isOpen = isControlled ? controlledOpen : internalOpen;

  const setOpen = (value: boolean) => {
    if (!isControlled) {
      setInternalOpen(value);
    }
    if (!value) {
      setQuery("");
    }
    onOpenChange?.(value);
  };

  const sortedActions = useMemo(
    () => actions.slice().sort((a, b) => a.label.localeCompare(b.label)),
    [actions],
  );

  const filteredActions = useMemo(() => {
    if (!query.trim()) {
      return sortedActions;
    }
    const term = query.toLowerCase();
    return sortedActions.filter((action) =>
      action.label.toLowerCase().includes(term),
    );
  }, [sortedActions, query]);

  useHotkeys({
    shortcut: "meta+k",
    handler: () => setOpen(!isOpen),
  });

  useHotkeys({
    shortcut: "ctrl+k",
    handler: () => setOpen(!isOpen),
  });

  useHotkeys({
    shortcut: "escape",
    enabled: isOpen,
    handler: () => setOpen(false),
  });

  const handleSelect = (action: CommandPaletteAction) => {
    action.handler();
    setOpen(false);
  };

  return (
    <CommandPaletteOverlay
      isOpen={isOpen}
      onClose={() => setOpen(false)}
      query={query}
      onQueryChange={setQuery}
      actions={filteredActions}
      onSelect={handleSelect}
    />
  );
};

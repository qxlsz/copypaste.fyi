import { useEffect, useMemo, useState } from "react";
import { Command } from "lucide-react";
import { cn } from "@/lib/utils";
import { useHotkeys } from "@/hooks/use-hotkeys";

export type CommandAction = {
  id: string;
  label: string;
  group: string;
  shortcut?: string;
  handler: () => void;
};

type Props = {
  actions: CommandAction[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function CommandPalette({ actions, open, onOpenChange }: Props) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setActive(0);
    }
  }, [open]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return actions;
    return actions.filter((action) => action.label.toLowerCase().includes(needle));
  }, [actions, query]);

  useHotkeys({
    shortcut: "mod+k",
    handler: () => onOpenChange(!open),
  });
  useHotkeys({
    shortcut: "esc",
    handler: () => onOpenChange(false),
    enabled: open,
  });

  if (!open) return null;

  const groups = filtered.reduce<Record<string, CommandAction[]>>((acc, action) => {
    acc[action.group] ??= [];
    acc[action.group].push(action);
    return acc;
  }, {});

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center bg-background/60 sm:items-start sm:px-4 sm:pt-[15vh]">
      <button
        type="button"
        className="absolute inset-0 cursor-default"
        aria-label="Dismiss command menu"
        onClick={() => onOpenChange(false)}
      />
      <div
        role="dialog"
        aria-label="Command menu"
        className="relative w-full overflow-hidden border-t border-border bg-background pb-[env(safe-area-inset-bottom)] sm:max-w-lg sm:border sm:pb-0"
      >
        <div className="flex items-center gap-2 border-b border-border px-3">
          <Command className="size-4 text-muted-foreground" aria-hidden="true" />
          <input
            autoFocus
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setActive(0);
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setActive((index) => Math.min(index + 1, filtered.length - 1));
              } else if (event.key === "ArrowUp") {
                event.preventDefault();
                setActive((index) => Math.max(index - 1, 0));
              } else if (event.key === "Enter") {
                event.preventDefault();
                const action = filtered[active];
                if (action) {
                  onOpenChange(false);
                  action.handler();
                }
              }
            }}
            placeholder="Jump to a command"
            className="h-12 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
          />
        </div>
        <div className="max-h-72 overflow-y-auto py-2">
          {filtered.length === 0 && (
            <p className="px-4 py-6 text-sm text-muted-foreground">No matching commands.</p>
          )}
          {Object.entries(groups).map(([group, items]) => (
            <div key={group} className="px-2 py-1">
              <p className="px-2 py-1 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
                {group}
              </p>
              {items.map((action) => {
                const index = filtered.indexOf(action);
                return (
                  <button
                    key={action.id}
                    type="button"
                    onClick={() => {
                      onOpenChange(false);
                      action.handler();
                    }}
                    className={cn(
                      "flex min-h-11 w-full items-center justify-between rounded-md px-2 py-2 text-left text-sm",
                      index === active
                        ? "bg-muted text-foreground"
                        : "text-muted-foreground hover:bg-muted hover:text-foreground",
                    )}
                  >
                    <span className="min-w-0 truncate">{action.label}</span>
                    {action.shortcut && (
                      <kbd className="hidden font-mono text-[10px] text-muted-foreground sm:inline">
                        {action.shortcut}
                      </kbd>
                    )}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

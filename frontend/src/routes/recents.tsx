import { useEffect, useState } from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { formatDistanceToNow } from "date-fns";
import { Flame, Lock, Trash2 } from "lucide-react";
import { formatLabel } from "@/lib/formats";
import { clearRecents, forgetPaste, listRecents, type RecentPaste } from "@/lib/recents";

export const Route = createFileRoute("/recents")({
  component: RecentsPage,
});

function createdLabel(iso: string) {
  const date = new Date(iso);
  if (!Number.isFinite(date.getTime())) return "";
  return formatDistanceToNow(date, { addSuffix: true });
}

function expiresLabel(iso: string | null) {
  if (!iso) return "no expiry";
  const date = new Date(iso);
  if (!Number.isFinite(date.getTime())) return "no expiry";
  return `expires ${formatDistanceToNow(date, { addSuffix: true })}`;
}

function RecentsPage() {
  const [items, setItems] = useState<RecentPaste[]>([]);

  useEffect(() => {
    setItems(listRecents());
  }, []);

  return (
    <section className="space-y-5">
      <header className="flex items-end justify-between gap-3">
        <div>
          <h1 className="text-xl font-medium tracking-tight">This device</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Recents live in local storage on this browser. They are not an account and are not
            synced.
          </p>
        </div>
        {items.length > 0 && (
          <button
            type="button"
            onClick={() => {
              clearRecents();
              setItems([]);
            }}
            className="inline-flex min-h-11 items-center border border-border px-3 text-xs text-muted-foreground transition-colors duration-150 hover:text-foreground"
          >
            Clear
          </button>
        )}
      </header>

      {items.length === 0 ? (
        <div className="border border-border px-5 py-10 text-sm text-muted-foreground">
          No pastes stored on this device yet.{" "}
          <Link to="/" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            Create one
          </Link>
          .
        </div>
      ) : (
        <ul className="divide-y divide-border border border-border">
          {items.map((item) => (
            <li key={item.id} className="flex items-start gap-3 px-4 py-3">
              <Link to="/p/$id" params={{ id: item.id }} className="min-w-0 flex-1">
                <p className="truncate font-mono text-sm text-foreground">{item.preview}</p>
                <p className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                  <span>{formatLabel(item.format)}</span>
                  <span aria-hidden="true">·</span>
                  <span>{createdLabel(item.createdAt)}</span>
                  <span aria-hidden="true">·</span>
                  <span>{expiresLabel(item.expiresAt)}</span>
                  {item.encrypted && (
                    <span className="inline-flex items-center gap-1">
                      <Lock className="size-3" />
                      encrypted
                    </span>
                  )}
                  {item.burnAfterReading && (
                    <span className="inline-flex items-center gap-1 text-danger">
                      <Flame className="size-3" />
                      burn
                    </span>
                  )}
                </p>
              </Link>
              <button
                type="button"
                aria-label="Remove from this device"
                onClick={() => {
                  forgetPaste(item.id);
                  setItems(listRecents());
                }}
                className="inline-flex size-11 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground sm:size-9"
              >
                <Trash2 className="size-4" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

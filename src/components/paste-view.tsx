import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { Copy, Download, Flame, GitFork, Link2, Lock, Type, WrapText } from "lucide-react";
import { toast } from "sonner";
import { decryptPaste, readKeyFromHash } from "@/lib/crypto-paste";
import { formatExtension, formatLabel } from "@/lib/formats";
import { tokenizeLine, tokenClass } from "@/lib/highlight";
import { renderMarkdown } from "@/lib/markdown";
import { readPaste, type PastePayload } from "@/lib/pastes";
import { cn, copyText, downloadText } from "@/lib/utils";

type Props = {
  id: string;
  raw?: boolean;
};

const MAX_RENDER_LINES = 8000;
const decryptedCache = new Map<string, string>();

function statusCopy(status: "not_found" | "expired" | "burned") {
  if (status === "expired") {
    return {
      title: "This paste has expired",
      body: "Retention elapsed and the record was removed.",
    };
  }
  if (status === "burned") {
    return {
      title: "This paste already burned",
      body: "Burn-after-reading consumed it on a previous view.",
    };
  }
  return {
    title: "Paste not found",
    body: "No paste exists for this id, or it has already disappeared.",
  };
}

function relativeExpiry(iso: string | null): string | null {
  if (!iso) return null;
  const ms = new Date(iso).getTime() - Date.now();
  if (!Number.isFinite(ms) || ms <= 0) return "expired";
  const minutes = Math.round(ms / 60_000);
  if (minutes < 60) return `expires in ${Math.max(1, minutes)}m`;
  if (minutes < 1440) return `expires in ${Math.round(minutes / 60)}h`;
  return `expires in ${Math.round(minutes / 1440)}d`;
}

export function PasteView({ id, raw = false }: Props) {
  const navigate = useNavigate();
  const query = useQuery({
    queryKey: ["paste", id],
    queryFn: () => readPaste({ data: { id } }),
    staleTime: Infinity,
    gcTime: 60 * 60 * 1000,
    refetchOnMount: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    retry: false,
  });
  const [unlock, setUnlock] = useState<
    | { kind: "pending" }
    | { kind: "locked"; paste: PastePayload }
    | { kind: "ready"; paste: PastePayload; plaintext: string }
  >({ kind: "pending" });
  const [passphrase, setPassphrase] = useState("");
  const [wrap, setWrap] = useState(
    () => typeof window !== "undefined" && window.matchMedia("(max-width: 639px)").matches,
  );

  useEffect(() => {
    let cancelled = false;
    const result = query.data;
    if (!result) return;

    const unlockWith = async (paste: PastePayload, secret: string) => {
      if (!secret || !paste.salt || !paste.nonce) return false;
      try {
        const plaintext = await decryptPaste({
          content: paste.content,
          salt: paste.salt,
          nonce: paste.nonce,
          secret,
        });
        if (!cancelled) {
          decryptedCache.set(paste.id, plaintext);
          setUnlock({ kind: "ready", paste, plaintext });
        }
        return true;
      } catch {
        return false;
      }
    };

    if (result.status !== "ok") {
      setUnlock({ kind: "pending" });
      return;
    }

    const paste = result.paste;
    if (!paste.encrypted) {
      setUnlock({ kind: "ready", paste, plaintext: paste.content });
      return;
    }
    const cached = decryptedCache.get(paste.id);
    if (cached) {
      setUnlock({ kind: "ready", paste, plaintext: cached });
      return;
    }

    const tryHash = () => unlockWith(paste, readKeyFromHash());
    void (async () => {
      const opened = await tryHash();
      if (!opened && !cancelled) setUnlock({ kind: "locked", paste });
    })();

    const onHash = () => {
      void tryHash();
    };
    window.addEventListener("hashchange", onHash);
    const retries = [50, 150, 400].map((ms) => window.setTimeout(onHash, ms));
    return () => {
      cancelled = true;
      window.removeEventListener("hashchange", onHash);
      retries.forEach((timer) => window.clearTimeout(timer));
    };
  }, [query.data]);

  const unlockManual = async () => {
    if (unlock.kind !== "locked") return;
    const { paste } = unlock;
    if (!paste.salt || !paste.nonce) {
      toast.error("This paste is missing encryption material");
      return;
    }
    try {
      const plaintext = await decryptPaste({
        content: paste.content,
        salt: paste.salt,
        nonce: paste.nonce,
        secret: passphrase,
      });
      decryptedCache.set(paste.id, plaintext);
      setUnlock({ kind: "ready", paste, plaintext });
    } catch {
      toast.error("That key does not decrypt this paste");
    }
  };

  if (query.isPending || (query.data?.status === "ok" && unlock.kind === "pending")) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center" role="status">
        <span className="size-5 animate-spin rounded-full border-2 border-border border-t-foreground" />
      </div>
    );
  }

  if (query.isError) {
    return (
      <EmptyState
        title="Could not load this paste"
        body="The store did not respond. Check your connection and try again."
      />
    );
  }

  if (query.data && query.data.status !== "ok") {
    const copy = statusCopy(query.data.status);
    return <EmptyState title={copy.title} body={copy.body} />;
  }

  if (unlock.kind === "locked") {
    return (
      <section className="flex min-h-0 flex-1 flex-col items-center justify-center px-6">
        <div className="w-full max-w-sm space-y-4">
          <div className="flex items-center gap-2">
            <Lock className="size-4" />
            <h1 className="font-mono text-sm font-medium tracking-tight">Encrypted paste</h1>
          </div>
          <p className="text-sm text-muted-foreground">
            Ciphertext is stored without the key. Enter the shared secret, or open the full share URL
            that includes the fragment.
          </p>
          <label className="block font-mono text-2xs text-muted-foreground" htmlFor="paste-key">
            encryption key
          </label>
          <input
            id="paste-key"
            type="password"
            autoComplete="off"
            value={passphrase}
            onChange={(event) => setPassphrase(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void unlockManual();
            }}
            className="w-full min-h-12 border border-border bg-background px-3 py-2 font-mono text-base outline-none focus:border-foreground sm:min-h-10 sm:text-sm"
          />
          <button
            type="button"
            onClick={() => void unlockManual()}
            className="inline-flex min-h-12 w-full items-center justify-center bg-foreground px-4 text-sm font-medium text-background sm:min-h-10"
          >
            Decrypt
          </button>
        </div>
      </section>
    );
  }

  if (unlock.kind !== "ready") return null;

  const { paste, plaintext } = unlock;
  const expiry = relativeExpiry(paste.expiresAt);
  const filename = `paste-${paste.id.slice(0, 8)}.${formatExtension(paste.format)}`;

  return (
    <article className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-auto">
        {raw || paste.format === "plain_text" ? (
          <pre
            className={cn(
              "min-h-full px-4 py-4 font-mono text-sm leading-7 sm:px-6 sm:py-5",
              wrap ? "whitespace-pre-wrap break-words" : "overflow-x-auto whitespace-pre",
            )}
          >
            {plaintext}
          </pre>
        ) : paste.format === "markdown" ? (
          <div
            className="px-5 py-4 sm:px-8 sm:py-6"
            dangerouslySetInnerHTML={{ __html: renderMarkdown(plaintext) }}
          />
        ) : (
          <CodeBlock format={paste.format} plaintext={plaintext} wrap={wrap} />
        )}
      </div>

      <footer className="shrink-0 border-t border-border bg-gutter pb-[max(0.5rem,env(safe-area-inset-bottom))] sm:pb-0">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 px-3 py-1.5 font-mono text-2xs text-muted-foreground sm:h-10 sm:pr-3">
          <span className="text-foreground">{formatLabel(paste.format)}</span>
          {expiry && (
            <>
              <span aria-hidden="true">·</span>
              <span>{expiry}</span>
            </>
          )}
          {paste.viewCount > 0 && (
            <>
              <span aria-hidden="true">·</span>
              <span>
                {paste.viewCount} {paste.viewCount === 1 ? "view" : "views"}
              </span>
            </>
          )}
          {paste.encrypted && (
            <span className="inline-flex items-center gap-1">
              <Lock className="size-3" />
              aes
            </span>
          )}
          {paste.burnAfterReading && (
            <span className="inline-flex items-center gap-1 text-danger">
              <Flame className="size-3" />
              burned
            </span>
          )}
          <div className="ml-auto hidden items-center sm:flex">
            <IconAction
              label="Copy content"
              onClick={async () => {
                const ok = await copyText(plaintext);
                if (ok) toast.success("Copied");
                else toast.error("Unable to copy");
              }}
            >
              <Copy className="size-4" />
            </IconAction>
            <IconAction
              label="Copy share URL"
              onClick={async () => {
                const ok = await copyText(window.location.href);
                if (ok) toast.success("Share URL copied");
                else toast.error("Unable to copy");
              }}
            >
              <Link2 className="size-4" />
            </IconAction>
            <IconAction
              label="Download"
              onClick={() => downloadText(filename, plaintext)}
            >
              <Download className="size-4" />
            </IconAction>
            <IconAction
              label={wrap ? "No wrap" : "Wrap lines"}
              onClick={() => setWrap((value) => !value)}
            >
              <WrapText className="size-4" />
            </IconAction>
            <IconAction
              label="Fork paste"
              onClick={() => {
                sessionStorage.setItem(
                  "copypaste.fork",
                  JSON.stringify({ content: plaintext, format: paste.format }),
                );
                void navigate({ to: "/" });
              }}
            >
              <GitFork className="size-4" />
            </IconAction>
            {!raw && (
              <Link
                to="/raw/$id"
                params={{ id: paste.id }}
                hash={paste.encrypted ? readKeyFromHash() || undefined : undefined}
                className="inline-flex size-8 items-center justify-center text-muted-foreground transition-colors duration-150 hover:text-foreground"
                aria-label="Raw view"
                title="Raw"
              >
                <Type className="size-4" />
              </Link>
            )}
          </div>
        </div>
        <div className="flex items-center gap-2 px-3 pr-16 pb-2 sm:hidden">
          <button
            type="button"
            onClick={async () => {
              const ok = await copyText(plaintext);
              if (ok) toast.success("Copied");
              else toast.error("Unable to copy");
            }}
            className="inline-flex h-12 flex-1 items-center justify-center gap-2 bg-foreground text-sm font-medium text-background"
          >
            <Copy className="size-4" />
            Copy
          </button>
          <button
            type="button"
            onClick={() => {
              sessionStorage.setItem(
                "copypaste.fork",
                JSON.stringify({ content: plaintext, format: paste.format }),
              );
              void navigate({ to: "/" });
            }}
            className="inline-flex size-12 shrink-0 items-center justify-center border border-border text-muted-foreground"
            aria-label="Fork paste"
            title="Fork"
          >
            <GitFork className="size-4" />
          </button>
          <button
            type="button"
            onClick={async () => {
              const ok = await copyText(window.location.href);
              if (ok) toast.success("Share URL copied");
              else toast.error("Unable to copy");
            }}
            className="inline-flex size-12 shrink-0 items-center justify-center border border-border text-muted-foreground"
            aria-label="Copy share URL"
            title="Share URL"
          >
            <Link2 className="size-4" />
          </button>
        </div>
      </footer>
    </article>
  );
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <section className="flex min-h-0 flex-1 flex-col items-center justify-center px-6">
      <div className="w-full max-w-sm space-y-3">
        <h1 className="font-mono text-sm font-medium tracking-tight">{title}</h1>
        <p className="text-sm text-muted-foreground">{body}</p>
        <Link
          to="/"
          className="inline-flex underline decoration-border underline-offset-4 hover:decoration-foreground"
        >
          Create a new paste
        </Link>
      </div>
    </section>
  );
}

function IconAction({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      className="inline-flex size-11 items-center justify-center text-muted-foreground transition-colors duration-150 hover:text-foreground sm:size-8"
    >
      {children}
    </button>
  );
}

function CodeBlock({
  format,
  plaintext,
  wrap,
}: {
  format: string;
  plaintext: string;
  wrap: boolean;
}) {
  const { shown, total } = useMemo(() => {
    const all = plaintext.split("\n");
    return {
      shown: all.length > MAX_RENDER_LINES ? all.slice(0, MAX_RENDER_LINES) : all,
      total: all.length,
    };
  }, [plaintext]);
  const rendered = useMemo(
    () => shown.map((line) => tokenizeLine(line, format)),
    [format, shown],
  );
  const truncated = total > MAX_RENDER_LINES;
  return (
    <div className="space-y-2">
      {truncated && (
        <p className="text-xs text-muted-foreground">
          Showing the first {MAX_RENDER_LINES.toLocaleString()} of {total.toLocaleString()}{" "}
          lines. Download the paste to read the rest.
        </p>
      )}
      <div
        className={cn(
          "grid min-h-full overflow-hidden",
          wrap ? "grid-cols-1" : "grid-cols-[auto_minmax(0,1fr)]",
        )}
      >
        {!wrap && (
          <div
            aria-hidden="true"
            className="select-none border-r border-border bg-gutter px-2 py-4 text-right font-mono text-2xs leading-7 text-muted-foreground tabular-nums whitespace-pre sm:px-3 sm:py-5"
          >
            {shown.map((_, index) => index + 1).join("\n")}
          </div>
        )}
        <pre
          className={cn(
            "px-4 py-4 font-mono text-sm leading-7 sm:px-5 sm:py-5",
            wrap ? "whitespace-pre-wrap break-words" : "overflow-x-auto whitespace-pre",
          )}
        >
          {rendered.map((tokens, index) => (
            <div key={index}>
              {wrap && (
                <span className="mr-4 hidden w-8 select-none text-right text-xs text-muted-foreground sm:inline-block">
                  {index + 1}
                </span>
              )}
              {tokens.length === 0 ? (
                " "
              ) : (
                tokens.map((token, tokenIndex) => (
                  <span key={tokenIndex} className={cn(tokenClass(token.kind))}>
                    {token.text}
                  </span>
                ))
              )}
            </div>
          ))}
        </pre>
      </div>
    </div>
  );
}

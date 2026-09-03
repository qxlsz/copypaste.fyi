import { useEffect, useMemo, useState, type FormEvent } from "react";
import { Link, useLocation, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Copy, Download, ExternalLink, GitFork, Share2 } from "lucide-react";
import { toast } from "sonner";

import { ApiError, fetchPaste, rawPasteUrl } from "../api/client";
import type { PasteViewResponse } from "../server/types";
import { MonacoEditor } from "../components/editor/MonacoEditor";
import { LostPaste } from "../components/LostPaste";
import { OpenWithAgents } from "../components/OpenWithAgents";
import { formatCountdown } from "../lib/countdown";
import { publicPasteUrl } from "../lib/openAgents";
import { sharePayload } from "../lib/whisper";

const formatLabel = (format: string) => {
  switch (format) {
    case "plain_text":
      return "Plain Text";
    case "markdown":
      return "Markdown";
    case "code":
      return "Code";
    case "json":
      return "JSON";
    case "javascript":
      return "JavaScript";
    case "typescript":
      return "TypeScript";
    case "python":
      return "Python";
    case "rust":
      return "Rust";
    case "go":
      return "Go";
    case "cpp":
      return "C++";
    case "kotlin":
      return "Kotlin";
    case "java":
      return "Java";
    case "csharp":
      return "C#";
    case "php":
      return "PHP";
    case "ruby":
      return "Ruby";
    case "bash":
      return "Bash";
    case "yaml":
      return "YAML";
    case "sql":
      return "SQL";
    case "swift":
      return "Swift";
    case "html":
      return "HTML";
    case "css":
      return "CSS";
    default:
      return format;
  }
};

const formatEncryption = (
  requiresKey: boolean,
  algorithm: PasteViewResponse["encryption"]["algorithm"],
) => {
  if (!requiresKey) {
    return "Plaintext";
  }
  switch (algorithm) {
    case "aes256_gcm":
      return "AES-256-GCM";
    case "chacha20_poly1305":
      return "ChaCha20-Poly1305";
    case "xchacha20_poly1305":
      return "XChaCha20-Poly1305";
    default:
      return algorithm;
  }
};

const formatTimeLock = (timeLock?: PasteViewResponse["timeLock"]) => {
  if (!timeLock) return "Not configured";
  const parts: string[] = [];
  if (timeLock.notBefore) {
    parts.push(`After ${new Date(timeLock.notBefore * 1000).toLocaleString()}`);
  }
  if (timeLock.notAfter) {
    parts.push(`Before ${new Date(timeLock.notAfter * 1000).toLocaleString()}`);
  }
  return parts.length > 0 ? parts.join(" · ") : "Configured";
};

// Extract the encryption key from a URL fragment of the form `#key=...`.
const parseHashKey = (hash: string): string | undefined => {
  if (!hash) return undefined;
  const params = new URLSearchParams(hash.replace(/^#/, ""));
  const key = params.get("key");
  return key ?? undefined;
};

const iconActionClasses =
  "inline-flex size-11 items-center justify-center rounded-lg text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none sm:size-10";

const useHasExpired = (expiresAt: number | null | undefined): boolean => {
  const [now, setNow] = useState(() => Date.now());
  const expired = typeof expiresAt === "number" && now >= expiresAt * 1000;

  useEffect(() => {
    if (typeof expiresAt !== "number" || expired) {
      return;
    }
    const delay = Math.max(0, expiresAt * 1000 - Date.now());
    const timeoutId = window.setTimeout(() => setNow(Date.now()), delay);
    return () => window.clearTimeout(timeoutId);
  }, [expiresAt, expired]);

  return expired;
};

// Live "expires in …" countdown for the metadata row. Ticks every second
// under an hour (seconds are visible), every minute otherwise, and stops
// once the paste has expired.
const ExpiryCountdown = ({ expiresAt }: { expiresAt: number }) => {
  const [now, setNow] = useState(() => Date.now());
  const remainingMs = expiresAt * 1000 - now;
  const expired = remainingMs <= 0;
  const underHour = remainingMs < 3_600_000;

  useEffect(() => {
    if (expired) {
      return;
    }
    const intervalId = window.setInterval(() => setNow(Date.now()), underHour ? 1_000 : 60_000);
    return () => window.clearInterval(intervalId);
  }, [expired, underHour]);

  const absolute = new Date(expiresAt * 1000).toLocaleString();
  if (expired) {
    return (
      <span className="text-danger" title={absolute}>
        expired
      </span>
    );
  }
  return <span title={absolute}>expires in {formatCountdown(remainingMs)}</span>;
};

const PasteViewSkeleton = () => (
  <div
    className="flex min-h-0 flex-1 items-center justify-center"
    role="status"
    aria-label="Loading paste"
  >
    <span className="h-5 w-5 animate-spin rounded-full border-2 border-border border-t-accent" />
  </div>
);

const EmptyState = ({ title, body }: { title: string; body: string }) => (
  <section className="flex min-h-0 flex-1 flex-col items-center justify-center px-6">
    <div className="w-full max-w-sm space-y-4">
      <h1 className="text-2xl font-medium tracking-tight text-text">{title}</h1>
      <p className="text-sm leading-relaxed text-muted-foreground">{body}</p>
      <Link
        to="/"
        className="inline-flex h-11 items-center rounded-md bg-accent px-4 text-sm font-medium text-accent-foreground"
      >
        New paste
      </Link>
    </div>
  </section>
);

export const PasteViewPage = () => {
  const { id } = useParams<{ id: string }>();
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [searchParams] = useSearchParams();
  const legacyQueryKey = searchParams.get("key") ?? undefined;
  // Prefer the fragment; accept legacy `?key=` links only as input. API
  // requests always carry the key in X-Paste-Key, never in their URL.
  const key = parseHashKey(location.hash) ?? legacyQueryKey;
  const [enteredKey, setEnteredKey] = useState(() => key ?? "");

  useEffect(() => {
    if (!legacyQueryKey) return;

    const sanitizedSearch = new URLSearchParams(location.search);
    sanitizedSearch.delete("key");
    const fragment = new URLSearchParams(location.hash.replace(/^#/, ""));
    if (!fragment.has("key")) {
      fragment.set("key", legacyQueryKey);
    }
    const nextSearch = sanitizedSearch.toString();
    navigate(
      {
        pathname: location.pathname,
        search: nextSearch ? `?${nextSearch}` : "",
        hash: fragment.toString(),
      },
      { replace: true },
    );
  }, [legacyQueryKey, location.hash, location.pathname, location.search, navigate]);

  useEffect(() => {
    setEnteredKey(key ?? "");
  }, [id, key]);

  const handleKeySubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const trimmed = enteredKey.trim();
    if (trimmed) {
      const sanitizedSearch = new URLSearchParams(location.search);
      sanitizedSearch.delete("key");
      const nextSearch = sanitizedSearch.toString();
      // Write the key into the fragment so subsequent navigation keeps it out
      // of HTTP request targets and Referer headers.
      navigate(
        {
          pathname: location.pathname,
          search: nextSearch ? `?${nextSearch}` : "",
          hash: `key=${encodeURIComponent(trimmed)}`,
        },
        { replace: true },
      );
    }
  };

  const sensitiveQueryPrefix = useMemo(() => ["paste", id] as const, [id]);
  const queryKey = useMemo(
    () => [...sensitiveQueryPrefix, key ? `key-attempt:${location.key}` : "without-key"],
    [key, location.key, sensitiveQueryPrefix],
  );

  const { data, isLoading, isError, error } = useQuery({
    // Sanitize legacy query-string keys before making any application fetch or
    // enabling share controls. This also avoids a second read of burn links
    // when React Router replaces the location.
    enabled: Boolean(id) && !legacyQueryKey,
    retry: false,
    staleTime: Infinity,
    gcTime: 0,
    refetchOnMount: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    queryKey,
    queryFn: () => fetchPaste(id!, key),
  });

  const clientExpired = useHasExpired(data?.expiresAt);

  useEffect(() => {
    const clearSensitiveQuery = () => {
      queryClient.removeQueries({
        queryKey: sensitiveQueryPrefix,
        exact: false,
      });
    };

    window.addEventListener("pagehide", clearSensitiveQuery);
    return () => {
      window.removeEventListener("pagehide", clearSensitiveQuery);
      clearSensitiveQuery();
    };
  }, [queryClient, sensitiveQueryPrefix]);

  const handleCopyContent = async () => {
    if (!data?.content) return;
    try {
      await navigator.clipboard.writeText(data.content);
      toast.success("Content copied to clipboard");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      toast.error("Unable to copy content", { description: message });
    }
  };

  const handleShare = async () => {
    const url = publicPasteUrl(`${window.location.origin}/p/${id}`);
    if (typeof navigator.share === "function") {
      try {
        await navigator.share(sharePayload(url));
      } catch (err) {
        // The user dismissing the share sheet is not an error worth surfacing.
        if (err instanceof Error && err.name === "AbortError") return;
        const message = err instanceof Error ? err.message : "Unknown error";
        toast.error("Unable to share link", { description: message });
      }
      return;
    }
    try {
      await navigator.clipboard.writeText(url);
      toast.success("Link copied to clipboard");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      toast.error("Unable to copy link", { description: message });
    }
  };

  const handleFork = () => {
    if (!data) return;
    navigate("/", {
      state: { content: data.content, format: data.format },
    });
  };

  const handleDownload = () => {
    if (!data?.content) return;
    const blob = new Blob([data.content], {
      type: "text/plain;charset=utf-8",
    });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `copypaste-${data.id}.txt`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  if (!id) {
    return <LostPaste />;
  }

  if (isLoading) {
    return <PasteViewSkeleton />;
  }

  if (isError || !data) {
    const message = error instanceof Error ? error.message : "Unknown error";
    const isBackendDown = message.includes("timed out") || message.includes("Failed to fetch");
    const attestationRequired =
      error instanceof ApiError &&
      (error.code === "attestation_required" || error.code === "attestation_invalid");
    const keyRequired =
      error instanceof ApiError &&
      (error.code === "key_required" || (error.status === 401 && !error.code));
    const keyRejected =
      error instanceof ApiError &&
      (error.code === "invalid_key" || (error.status === 403 && !error.code));

    if (attestationRequired) {
      return (
        <EmptyState
          title="Additional verification required"
          body="This paste uses a legacy attestation gate that this web client does not submit through URL parameters. Ask the sender for a supported sharing method."
        />
      );
    }

    if (keyRequired || keyRejected) {
      return (
        <section className="flex min-h-0 flex-1 flex-col items-center justify-center px-6">
          <form onSubmit={handleKeySubmit} className="w-full max-w-sm space-y-5">
            <div className="space-y-2">
              <h1 className="text-2xl font-medium tracking-tight text-text">Encrypted paste</h1>
              <p className="text-sm leading-relaxed text-muted-foreground">
                Ciphertext is stored without the key. Enter the shared secret, or open the full
                share URL.
              </p>
            </div>
            <div className="space-y-1.5">
              <label className="block text-xs font-medium text-muted-foreground" htmlFor="pasteKey">
                encryption key
              </label>
              <input
                id="pasteKey"
                type="password"
                value={enteredKey}
                onChange={(event) => setEnteredKey(event.target.value)}
                autoCapitalize="none"
                autoComplete="off"
                autoCorrect="off"
                spellCheck={false}
                placeholder="Shared secret"
                className="min-h-12 w-full rounded-md bg-surface px-3 font-mono text-base text-text shadow-soft placeholder:text-muted-foreground focus:outline-none focus:shadow-strong sm:min-h-11 sm:text-sm"
                required
                autoFocus
              />
              {keyRejected && key && (
                <p className="text-sm text-danger">That key does not decrypt this paste.</p>
              )}
            </div>
            <button
              type="submit"
              className="inline-flex h-12 w-full items-center justify-center rounded-md bg-accent text-sm font-medium text-accent-foreground sm:h-11"
            >
              Decrypt
            </button>
          </form>
        </section>
      );
    }

    const isAbsence =
      error instanceof ApiError &&
      (error.status === 404 ||
        error.status === 410 ||
        error.code === "paste_not_found" ||
        error.code === "gone");
    if (isAbsence) {
      return <LostPaste seed={id} />;
    }

    return (
      <EmptyState
        title={isBackendDown ? "Could not load this paste" : "Unable to load paste"}
        body={
          isBackendDown
            ? "The store did not respond. Check your connection and try again."
            : message
        }
      />
    );
  }

  if (clientExpired) {
    return <LostPaste seed={id} />;
  }

  return (
    <article className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1">
        <MonacoEditor
          value={data.content}
          format={data.format}
          readOnly
          height="100%"
          className="min-h-0 h-full w-full"
        />
      </div>

      <footer className="shrink-0 border-t border-border bg-surface pb-[max(0.75rem,env(safe-area-inset-bottom))]">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 px-3 py-2 text-xs text-muted-foreground">
          <span className="text-text">{formatLabel(data.format)}</span>
          {data.expiresAt ? (
            <>
              <span aria-hidden="true">·</span>
              <ExpiryCountdown expiresAt={data.expiresAt} />
            </>
          ) : (
            <>
              <span aria-hidden="true">·</span>
              <span>no expiry</span>
            </>
          )}
          {data.burnAfterReading ? <span className="text-danger">burn</span> : null}
          {data.encryption.requiresKey ? (
            <span>{formatEncryption(data.encryption.requiresKey, data.encryption.algorithm)}</span>
          ) : null}
          {data.timeLock ? (
            <>
              <span aria-hidden="true">·</span>
              <span>{formatTimeLock(data.timeLock)}</span>
            </>
          ) : null}
          <div className="ml-auto hidden items-center sm:flex">
            <button
              type="button"
              onClick={handleCopyContent}
              className={iconActionClasses}
              aria-label="Copy content"
              title="Copy content"
            >
              <Copy className="h-4 w-4" aria-hidden="true" />
            </button>
            {!data.encryption.requiresKey && !data.burnAfterReading ? (
              <a
                href={rawPasteUrl(data.id)}
                target="_blank"
                rel="noopener noreferrer"
                className={iconActionClasses}
                aria-label="Open raw plaintext"
                title="Raw"
              >
                <ExternalLink className="h-4 w-4" aria-hidden="true" />
              </a>
            ) : null}
            <button
              type="button"
              onClick={handleDownload}
              className={iconActionClasses}
              aria-label="Download content"
              title="Download"
            >
              <Download className="h-4 w-4" aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={handleShare}
              className={iconActionClasses}
              aria-label="Share link"
              title="Share"
            >
              <Share2 className="h-4 w-4" aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={handleFork}
              className={iconActionClasses}
              aria-label="New paste from this content"
              title="Fork into a new paste"
            >
              <GitFork className="h-4 w-4" aria-hidden="true" />
            </button>
          </div>
        </div>
        <div className="flex items-center gap-2 px-3 pb-2 sm:hidden">
          <button
            type="button"
            onClick={handleCopyContent}
            className="inline-flex h-12 flex-1 items-center justify-center gap-2 rounded-lg bg-accent text-sm font-medium text-accent-foreground"
          >
            <Copy className="h-4 w-4" aria-hidden="true" />
            Copy
          </button>
          <button
            type="button"
            onClick={handleShare}
            className="inline-flex h-12 flex-1 items-center justify-center gap-2 rounded-lg bg-muted text-sm font-medium text-text"
            aria-label="Share link"
          >
            <Share2 className="h-4 w-4" aria-hidden="true" />
            Share
          </button>
          <button
            type="button"
            onClick={handleFork}
            className="inline-flex size-12 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground"
            aria-label="New paste from this content"
            title="Fork"
          >
            <GitFork className="h-4 w-4" aria-hidden="true" />
          </button>
        </div>
        <div className="px-3 pb-2">
          <OpenWithAgents url={`${window.location.origin}/p/${data.id}`} />
        </div>
      </footer>
    </article>
  );
};

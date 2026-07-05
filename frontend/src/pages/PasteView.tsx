import { useEffect, useMemo, useState, type FormEvent } from "react";
import {
  useLocation,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { Copy, Download, ExternalLink, GitFork, Share2 } from "lucide-react";
import { toast } from "sonner";

import { ApiError, fetchPaste } from "../api/client";
import type { PasteViewResponse } from "../server/types";
import { MonacoEditor } from "../components/editor/MonacoEditor";
import { formatCountdown } from "../lib/countdown";

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

const formatAttestation = (attestation?: PasteViewResponse["attestation"]) => {
  if (!attestation) return "None";
  if (attestation.kind === "totp") {
    return attestation.issuer ? `TOTP (${attestation.issuer})` : "TOTP";
  }
  if (attestation.kind === "shared_secret") {
    return "Shared secret";
  }
  return attestation.kind;
};

const formatPersistence = (persistence?: PasteViewResponse["persistence"]) => {
  if (!persistence) return "Ephemeral (memory)";
  if (persistence.detail) {
    return `${persistence.kind} · ${persistence.detail}`;
  }
  return persistence.kind;
};

const formatWebhook = (webhook?: PasteViewResponse["webhook"]) => {
  if (!webhook) return "None";
  switch (webhook.provider) {
    case "slack":
      return "Slack";
    case "teams":
      return "Microsoft Teams";
    case "generic":
      return "Webhook";
    default:
      return "Webhook";
  }
};

// Extract the encryption key from a URL fragment of the form `#key=...`.
const parseHashKey = (hash: string): string | undefined => {
  if (!hash) return undefined;
  const params = new URLSearchParams(hash.replace(/^#/, ""));
  const key = params.get("key");
  return key ?? undefined;
};

const iconActionClasses =
  "inline-flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface";

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
    const intervalId = window.setInterval(
      () => setNow(Date.now()),
      underHour ? 1_000 : 60_000,
    );
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
  return (
    <span title={absolute}>expires in {formatCountdown(remainingMs)}</span>
  );
};

const PasteViewSkeleton = () => (
  <div className="animate-pulse space-y-6" aria-hidden="true">
    <div className="space-y-2">
      <div className="h-5 w-48 rounded bg-muted" />
      <div className="h-3 w-72 rounded bg-muted" />
    </div>
    <div className="rounded-lg border border-border bg-surface p-4">
      <div className="h-64 rounded-md bg-muted" />
    </div>
    <div className="rounded-lg border border-border bg-surface p-4">
      <div className="h-5 w-32 rounded bg-muted" />
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        <div className="h-10 rounded bg-muted" />
        <div className="h-10 rounded bg-muted" />
        <div className="h-10 rounded bg-muted" />
        <div className="h-10 rounded bg-muted" />
      </div>
    </div>
  </div>
);

export const PasteViewPage = () => {
  const { id } = useParams<{ id: string }>();
  const location = useLocation();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  // Prefer the fragment (never sent to servers); fall back to the legacy
  // `?key=` search param so old share links keep working.
  const key =
    parseHashKey(location.hash) ?? searchParams.get("key") ?? undefined;
  const [enteredKey, setEnteredKey] = useState(() => key ?? "");

  const handleKeySubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const trimmed = enteredKey.trim();
    if (trimmed) {
      // Write the key into the fragment (not the query string) so it stays
      // out of history syncing, referrers, and server logs.
      navigate(
        {
          pathname: location.pathname,
          search: location.search,
          hash: `key=${encodeURIComponent(trimmed)}`,
        },
        { replace: true },
      );
    }
  };

  const queryKey = useMemo(() => ["paste", id, key ?? null], [id, key]);

  const { data, isLoading, isError, error } = useQuery({
    enabled: Boolean(id),
    retry: false,
    queryKey,
    queryFn: () => fetchPaste(id!, key),
  });

  const stegoDataUrl = useMemo(() => {
    if (!data?.stego) return null;
    return `data:${data.stego.carrierMime};base64,${data.stego.carrierImage}`;
  }, [data?.stego]);

  const editorHeight = useMemo(() => {
    const lines = data?.content?.split("\n") ?? [];
    const lineCount = lines.length > 0 ? lines.length : 12;
    const clamped = Math.min(Math.max(lineCount, 12), 60);
    return `${clamped * 20}px`;
  }, [data?.content]);

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
    const url = window.location.href;
    if (typeof navigator.share === "function") {
      try {
        await navigator.share({ title: `copypaste.fyi — ${id}`, url });
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
    return (
      <div className="mx-auto max-w-3xl space-y-4 text-center">
        <h1 className="text-xl font-semibold tracking-tight text-text">
          Paste not found
        </h1>
        <p className="text-sm text-muted-foreground">
          The requested paste ID is missing or invalid.
        </p>
      </div>
    );
  }

  if (isLoading) {
    return <PasteViewSkeleton />;
  }

  if (isError || !data) {
    const message = error instanceof Error ? error.message : "Unknown error";
    const isBackendDown =
      message.includes("timed out") || message.includes("Failed to fetch");
    const keyRequired =
      error instanceof ApiError &&
      (error.status === 401 || error.code === "key_required");
    const keyRejected =
      error instanceof ApiError &&
      (error.status === 403 || error.code === "invalid_key");

    if (keyRequired || keyRejected) {
      return (
        <div className="mx-auto max-w-sm space-y-6 py-8">
          <div className="space-y-2 text-center">
            <h1 className="text-xl font-semibold tracking-tight text-text">
              Encrypted paste
            </h1>
            <p className="text-sm text-muted-foreground">
              This paste requires an encryption key to view.
            </p>
          </div>

          <form onSubmit={handleKeySubmit} className="space-y-4">
            <div className="space-y-1.5">
              <label
                className="block text-xs font-medium text-muted-foreground"
                htmlFor="pasteKey"
              >
                encryption key
              </label>
              <input
                id="pasteKey"
                type="password"
                value={enteredKey}
                onChange={(event) => setEnteredKey(event.target.value)}
                placeholder="Enter the encryption key…"
                className="w-full rounded-md border border-border bg-surface px-3 py-2 font-mono text-sm text-text placeholder:text-muted-foreground focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent"
                required
                autoFocus
              />
              {keyRejected && key && (
                <p className="text-xs text-danger">
                  The provided key was rejected. Please double-check and try
                  again.
                </p>
              )}
            </div>

            <button
              type="submit"
              className="w-full rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-foreground transition hover:bg-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
            >
              View paste
            </button>
          </form>

          <div className="text-center text-xs text-muted-foreground">
            <p>The key was provided when the paste was created.</p>
            <p>If you don't have the key, the paste cannot be viewed.</p>
          </div>
        </div>
      );
    }

    return (
      <div className="space-y-3">
        <h1 className="text-xl font-semibold tracking-tight text-danger">
          {isBackendDown ? "Backend unavailable" : "Unable to load paste"}
        </h1>
        <p className="text-sm text-muted-foreground">
          {isBackendDown
            ? "The paste service is currently unavailable. Please try again later or contact support if the issue persists."
            : message}
        </p>
        {isBackendDown && (
          <p className="text-xs text-muted-foreground">
            Make sure the backend server is running on port 8000.
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <header className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <h1 className="font-mono text-sm font-medium text-text">{data.id}</h1>
        <p className="flex flex-wrap items-center gap-x-2 font-mono text-xs text-muted-foreground">
          <span>{formatLabel(data.format)}</span>
          <span aria-hidden="true">·</span>
          <span>
            created {new Date(data.createdAt * 1000).toLocaleString()}
          </span>
          <span aria-hidden="true">·</span>
          {data.expiresAt ? (
            <ExpiryCountdown expiresAt={data.expiresAt} />
          ) : (
            <span>never expires</span>
          )}
        </p>
        {data.burnAfterReading ? (
          <span className="rounded border border-danger/40 bg-danger/10 px-1.5 py-0.5 font-mono text-[10px] text-danger">
            burn-after-read
          </span>
        ) : null}
        {data.encryption.requiresKey ? (
          <span className="rounded border border-accent/40 px-1.5 py-0.5 font-mono text-[10px] text-accent">
            {formatEncryption(
              data.encryption.requiresKey,
              data.encryption.algorithm,
            ).toLowerCase()}
          </span>
        ) : null}
      </header>

      <section className="overflow-hidden rounded-lg border border-border bg-surface">
        <div className="flex items-center justify-end gap-1 border-b border-border px-2 py-1.5">
          <button
            type="button"
            onClick={handleCopyContent}
            className={iconActionClasses}
            aria-label="Copy content"
            title="Copy content"
          >
            <Copy className="h-4 w-4" aria-hidden="true" />
          </button>
          <a
            href={`/p/${data.id}/raw`}
            target="_blank"
            rel="noopener noreferrer"
            className={iconActionClasses}
            aria-label="Open raw plaintext"
            title="Raw"
          >
            <ExternalLink className="h-4 w-4" aria-hidden="true" />
          </a>
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
        <MonacoEditor
          value={data.content}
          format={data.format}
          readOnly
          height={editorHeight}
        />
      </section>

      <details className="group rounded-lg border border-border bg-surface">
        <summary className="cursor-pointer select-none list-none px-4 py-2.5 text-xs font-medium uppercase tracking-wide text-muted-foreground transition hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent">
          details
        </summary>
        <dl className="grid gap-3 border-t border-border px-4 py-4 sm:grid-cols-2">
          <div>
            <dt className="text-xs uppercase tracking-wide text-muted-foreground">
              Encryption
            </dt>
            <dd className="font-mono text-sm text-text">
              {formatEncryption(
                data.encryption.requiresKey,
                data.encryption.algorithm,
              )}
            </dd>
          </div>
          {data.attestation ? (
            <div>
              <dt className="text-xs uppercase tracking-wide text-muted-foreground">
                Attestation
              </dt>
              <dd className="text-sm text-text">
                {formatAttestation(data.attestation)}
              </dd>
            </div>
          ) : null}
          {data.timeLock ? (
            <div>
              <dt className="text-xs uppercase tracking-wide text-muted-foreground">
                Time lock
              </dt>
              <dd className="font-mono text-sm text-text">
                {formatTimeLock(data.timeLock)}
              </dd>
            </div>
          ) : null}
          <div>
            <dt className="text-xs uppercase tracking-wide text-muted-foreground">
              Persistence
            </dt>
            <dd className="text-sm text-text">
              {formatPersistence(data.persistence)}
            </dd>
          </div>
          <div>
            <dt className="text-xs uppercase tracking-wide text-muted-foreground">
              Webhook
            </dt>
            <dd className="text-sm text-text">{formatWebhook(data.webhook)}</dd>
          </div>
          {data.bundle?.children?.length ? (
            <div>
              <dt className="text-xs uppercase tracking-wide text-muted-foreground">
                Bundle shares
              </dt>
              <dd className="text-sm text-text">
                {data.bundle.children.length}
              </dd>
            </div>
          ) : null}
        </dl>
      </details>

      {data.stego ? (
        <section className="rounded-lg border border-success/30 bg-success/5 p-4">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
            <div className="space-y-2">
              <h2 className="text-sm font-semibold tracking-tight text-text">
                Steganographic carrier
              </h2>
              <p className="text-xs text-muted-foreground">
                The encrypted payload is embedded in the carrier image below.
                Share this cover along with the encryption key to allow
                recipients to extract and decrypt the paste locally.
              </p>
              <dl className="mt-3 space-y-2 text-xs">
                <div>
                  <dt className="uppercase tracking-wide text-muted-foreground">
                    Mime type
                  </dt>
                  <dd className="font-mono text-text">
                    {data.stego.carrierMime}
                  </dd>
                </div>
                <div>
                  <dt className="uppercase tracking-wide text-muted-foreground">
                    Payload digest (SHA-256)
                  </dt>
                  <dd className="break-all font-mono text-text">
                    {data.stego.payloadDigest}
                  </dd>
                </div>
              </dl>
              {stegoDataUrl ? (
                <a
                  href={stegoDataUrl}
                  download={`copypaste-stego-${data.id}.png`}
                  className="inline-flex items-center gap-2 rounded-md border border-border bg-surface px-3 py-1.5 text-xs font-medium text-text transition hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
                >
                  <Download className="h-3.5 w-3.5" aria-hidden="true" />
                  Download carrier image
                </a>
              ) : null}
            </div>
            {stegoDataUrl ? (
              <div className="overflow-hidden rounded-md border border-border bg-surface">
                <img
                  src={stegoDataUrl}
                  alt="Steganographic carrier"
                  className="max-h-64 w-full object-contain"
                />
              </div>
            ) : null}
          </div>
        </section>
      ) : null}
    </div>
  );
};

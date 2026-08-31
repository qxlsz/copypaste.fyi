import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { useLocation } from "react-router-dom";
import { useMutation } from "@tanstack/react-query";
import { toast } from "sonner";
import QRCode from "qrcode";
import {
  Check,
  ChevronDown,
  Copy,
  Flame,
  Lock,
  QrCode,
  Share2,
} from "lucide-react";

import { createPaste } from "../api/client";
import type {
  CreatePastePayload,
  EncryptionAlgorithm,
  PasteFormat,
} from "../api/types";
import { MonacoEditor } from "../components/editor/MonacoEditor";
import { useHotkeys } from "../hooks/useHotkeys";
import {
  buildPasteShareUrl,
  generateEncryptionKey,
  validateEncryptionKey,
} from "../lib/pasteSecurity";
import { useAuth } from "../stores/auth";

const formatOptions: Array<{ label: string; value: PasteFormat }> = [
  { label: "Plain text", value: "plain_text" },
  { label: "Markdown", value: "markdown" },
  { label: "Generic code", value: "code" },
  { label: "JSON", value: "json" },
  { label: "JavaScript", value: "javascript" },
  { label: "TypeScript", value: "typescript" },
  { label: "Python", value: "python" },
  { label: "Rust", value: "rust" },
  { label: "Go", value: "go" },
  { label: "C++", value: "cpp" },
  { label: "Kotlin", value: "kotlin" },
  { label: "Java", value: "java" },
  { label: "C#", value: "csharp" },
  { label: "PHP", value: "php" },
  { label: "Ruby", value: "ruby" },
  { label: "Bash", value: "bash" },
  { label: "YAML", value: "yaml" },
  { label: "SQL", value: "sql" },
  { label: "Swift", value: "swift" },
  { label: "HTML", value: "html" },
  { label: "CSS", value: "css" },
];

const encryptionOptions: Array<{ label: string; value: EncryptionAlgorithm }> =
  [
    { label: "None", value: "none" },
    { label: "AES-256-GCM", value: "aes256_gcm" },
    { label: "ChaCha20-Poly1305", value: "chacha20_poly1305" },
    { label: "XChaCha20-Poly1305", value: "xchacha20_poly1305" },
    {
      label: "Kyber Hybrid AES-256-GCM (Post-Quantum)",
      value: "kyber_hybrid_aes256_gcm",
    },
  ];

const encryptionChipLabel: Record<EncryptionAlgorithm, string> = {
  none: "",
  aes256_gcm: "aes-256-gcm",
  chacha20_poly1305: "chacha20-poly1305",
  xchacha20_poly1305: "xchacha20-poly1305",
  kyber_hybrid_aes256_gcm: "kyber-hybrid",
};

const retentionOptions: Array<{ label: string; value: number }> = [
  { label: "1m", value: 1 },
  { label: "10m", value: 10 },
  { label: "1h", value: 60 },
  { label: "3h", value: 180 },
  { label: "1d", value: 1440 },
  { label: "7d", value: 10080 },
  { label: "30d", value: 43200 },
];

const fieldLabelClasses = "block text-xs font-medium text-muted-foreground";

const inputClasses =
  "w-full rounded-md border border-border bg-surface px-3 py-2 text-sm text-text placeholder:text-muted-foreground focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent disabled:cursor-not-allowed disabled:opacity-50";

// State passed via `navigate("/", { state })` when forking an existing paste.
interface ForkState {
  content?: unknown;
  format?: unknown;
}

const isPasteFormat = (value: unknown): value is PasteFormat =>
  typeof value === "string" &&
  formatOptions.some((option) => option.value === value);

export const PasteFormPage = () => {
  const { token } = useAuth();
  const location = useLocation();
  // Seed the editor from router state (fork flow) on mount only; the lazy
  // initializers never re-run, so later navigation state changes can't loop.
  const [content, setContent] = useState(() => {
    const state = location.state as ForkState | null;
    return typeof state?.content === "string" ? state.content : "";
  });
  const [format, setFormat] = useState<PasteFormat>(() => {
    const state = location.state as ForkState | null;
    return isPasteFormat(state?.format) ? state.format : "plain_text";
  });
  const [retentionMinutes, setRetentionMinutes] = useState<number>(1440);
  const [encryption, setEncryption] = useState<EncryptionAlgorithm>("none");
  const [encryptionKey, setEncryptionKey] = useState("");
  const [writeCredential, setWriteCredential] = useState("");
  const [burnAfterReading, setBurnAfterReading] = useState(false);
  const [shareUrl, setShareUrl] = useState<string | null>(null);
  const [isCopying, setIsCopying] = useState(false);
  const [showQr, setShowQr] = useState(false);
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [pasteEncryption, setPasteEncryption] =
    useState<EncryptionAlgorithm>("none");
  const [pasteEncryptionKey, setPasteEncryptionKey] = useState("");
  const [isEncryptionOpen, setEncryptionOpen] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);

  const mutation = useMutation({
    mutationFn: async () => {
      const payload: CreatePastePayload = {
        content,
        format,
        retention_minutes: retentionMinutes
          ? Number(retentionMinutes)
          : undefined,
        burn_after_reading: burnAfterReading || undefined,
      };

      if (encryption !== "none") {
        payload.encryption = {
          algorithm: encryption,
          key: encryptionKey,
        };
      }

      return createPaste(payload, {
        sessionToken: token,
        writeCredential: writeCredential.trim() || undefined,
      });
    },
    onSuccess: (result) => {
      const usedEncryption = encryption;
      const usedEncryptionKey = encryptionKey;
      toast.success("Paste created");
      // Store the encryption settings used for this paste
      setPasteEncryption(usedEncryption);
      setPasteEncryptionKey(usedEncryptionKey);
      setContent("");
      setShareUrl(result.shareableUrl);
      setEncryptionKey("");
      if (usedEncryption !== "none") {
        setEncryption("none");
      }
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Failed to create paste", { description: message });
    },
  });

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!content.trim()) {
      toast.error("Content is required");
      return;
    }
    if (encryption !== "none") {
      const encryptionKeyError = validateEncryptionKey(encryptionKey);
      if (encryptionKeyError) {
        toast.error("Invalid encryption key", {
          description: encryptionKeyError,
        });
        setEncryptionOpen(true);
        return;
      }
    }
    if (new TextEncoder().encode(writeCredential).byteLength > 4096) {
      toast.error("Invalid write credential", {
        description: "Write credentials must be 4096 bytes or smaller.",
      });
      return;
    }
    setShareUrl(null);
    mutation.mutate();
  };

  const submitForm = () => {
    formRef.current?.requestSubmit();
  };

  // ⌘⏎ / Ctrl+⏎ submits the composer from anywhere on the page.
  useHotkeys({ shortcut: "meta+enter", handler: submitForm });
  useHotkeys({ shortcut: "ctrl+enter", handler: submitForm });

  const requiresKey = encryption !== "none";

  const createSecureEncryptionKey = () => {
    try {
      return generateEncryptionKey();
    } catch {
      toast.error("Unable to generate a secure encryption key");
      return null;
    }
  };

  const handleGenerateEncryptionKey = () => {
    const key = createSecureEncryptionKey();
    if (!key) return;
    setEncryptionKey(key);
    if (encryption === "none") {
      setEncryption("aes256_gcm");
    }
    toast.message("Secure 256-bit encryption key generated", {
      description: "The key was added to the encryption field.",
    });
  };

  const shareLink = useMemo(() => {
    if (!shareUrl) {
      return null;
    }

    try {
      return buildPasteShareUrl(
        shareUrl,
        pasteEncryption !== "none" ? pasteEncryptionKey : "",
        window.location.origin,
      );
    } catch {
      return null;
    }
  }, [shareUrl, pasteEncryption, pasteEncryptionKey]);

  const handleCopyShareUrl = async () => {
    const urlToCopy = shareLink;
    if (!urlToCopy) return;
    try {
      setIsCopying(true);
      await navigator.clipboard.writeText(urlToCopy);
      toast.success("Link copied to clipboard");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Unable to copy link", { description: message });
    } finally {
      setIsCopying(false);
    }
  };

  const handleShareLink = async () => {
    const urlToShare = shareLink;
    if (!urlToShare) return;
    if (typeof navigator.share === "function") {
      try {
        await navigator.share({
          title: "copypaste.fyi paste",
          url: urlToShare,
        });
      } catch (error) {
        // The user dismissing the share sheet is not an error worth surfacing.
        if (error instanceof Error && error.name === "AbortError") return;
        const message =
          error instanceof Error ? error.message : "Unknown error";
        toast.error("Unable to share link", { description: message });
      }
      return;
    }
    await handleCopyShareUrl();
  };

  // Render the QR code lazily — only once the toggle is opened.
  useEffect(() => {
    if (!shareLink || !showQr) {
      setQrDataUrl(null);
      return;
    }
    let cancelled = false;
    QRCode.toDataURL(shareLink, { margin: 1, width: 160 })
      .then((dataUrl) => {
        if (!cancelled) setQrDataUrl(dataUrl);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setQrDataUrl(null);
        const message =
          error instanceof Error ? error.message : "Unknown error";
        toast.error("Unable to generate QR code", { description: message });
      });
    return () => {
      cancelled = true;
    };
  }, [shareLink, showQr]);

  return (
    <form
      ref={formRef}
      className="flex min-h-0 flex-1 flex-col"
      onSubmit={handleSubmit}
    >
      {shareLink && (
        <section
          className="shrink-0 space-y-3 border-b border-border bg-gutter px-4 py-3 sm:px-5"
          aria-label="Paste created"
        >
          <div className="flex items-center gap-2">
            <Check className="h-4 w-4 text-success" aria-hidden="true" />
            <h2 className="font-mono text-xs font-semibold tracking-tight text-text">
              Paste created
            </h2>
          </div>
          <div className="space-y-1.5">
            <label className={fieldLabelClasses} htmlFor="share-url">
              share url
            </label>
            <div className="flex gap-2">
              <input
                id="share-url"
                readOnly
                value={shareLink}
                onFocus={(event) => event.target.select()}
                className={`${inputClasses} min-h-11 font-mono text-xs sm:min-h-9`}
              />
              <button
                type="button"
                onClick={handleCopyShareUrl}
                disabled={isCopying}
                className="inline-flex size-11 flex-shrink-0 items-center justify-center border border-border text-muted-foreground transition hover:text-text disabled:opacity-60 sm:size-9"
                aria-label={isCopying ? "Copying link…" : "Copy link"}
                title="Copy link"
              >
                <Copy className="h-4 w-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                onClick={handleShareLink}
                className="inline-flex size-11 flex-shrink-0 items-center justify-center border border-border text-muted-foreground transition hover:text-text sm:size-9"
                aria-label="Share link"
                title="Share link"
              >
                <Share2 className="h-4 w-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                onClick={() => setShowQr((open) => !open)}
                aria-pressed={showQr}
                className={`inline-flex h-11 flex-shrink-0 items-center gap-1.5 border border-border px-2.5 font-mono text-[11px] transition sm:h-9 ${
                  showQr
                    ? "bg-text text-background"
                    : "text-muted-foreground hover:text-text"
                }`}
                aria-label={showQr ? "Hide QR code" : "Show QR code"}
                title={showQr ? "Hide QR code" : "Show QR code"}
              >
                <QrCode className="h-4 w-4" aria-hidden="true" />
                qr
              </button>
            </div>
            {showQr && qrDataUrl && (
              <div className="w-fit border border-border bg-surface p-2">
                <img
                  src={qrDataUrl}
                  alt="QR code for the paste share link"
                  width={160}
                  height={160}
                  className="block h-40 w-40"
                />
              </div>
            )}
            <a
              href={shareLink}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-block text-xs text-muted-foreground underline-offset-2 hover:text-text hover:underline"
            >
              Open paste
            </a>
          </div>
          {pasteEncryption !== "none" && pasteEncryptionKey && (
            <div className="space-y-1.5">
              <label className={fieldLabelClasses} htmlFor="share-key">
                encryption key — share out of band
              </label>
              <input
                id="share-key"
                readOnly
                value={pasteEncryptionKey}
                onFocus={(event) => event.target.select()}
                className={`${inputClasses} font-mono text-xs`}
              />
            </div>
          )}
        </section>
      )}

      <div className="shrink-0 border-b border-border bg-gutter px-3 py-2 sm:px-4">
        <label className={fieldLabelClasses} htmlFor="write-credential">
          write access credential
        </label>
        <input
          id="write-credential"
          type="password"
          value={writeCredential}
          onChange={(event) => setWriteCredential(event.target.value)}
          autoComplete="off"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          placeholder="Operator-issued token (required on public deploy)"
          title="Public deployments are closed by default. This credential stays only in this tab's memory and is never saved by the browser app."
          className="h-11 w-full border-0 bg-transparent px-0 font-mono text-base text-text outline-none placeholder:text-muted-foreground sm:h-8 sm:text-xs"
        />
      </div>

      <label className="sr-only" htmlFor="content">
        Content
      </label>
      <MonacoEditor
        value={content}
        onChange={setContent}
        format={format}
        height="100%"
        className="min-h-0 w-full flex-1"
      />

      <div className="shrink-0 border-t border-border bg-gutter pb-[max(0.5rem,env(safe-area-inset-bottom))] sm:pb-[max(0px,env(safe-area-inset-bottom))]">
        <div className="flex items-center gap-0.5 px-2 sm:h-10 sm:gap-1 sm:px-3">
          <div className="relative min-w-0 flex-1 sm:flex-none">
            <label className="sr-only" htmlFor="format">
              Format
            </label>
            <select
              id="format"
              value={format}
              onChange={(event) => setFormat(event.target.value as PasteFormat)}
              className="h-11 w-full min-w-0 appearance-none bg-transparent py-0 pl-1 pr-5 font-mono text-base text-text outline-none sm:h-8 sm:w-auto sm:text-xs"
            >
              {formatOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <ChevronDown
              className="pointer-events-none absolute right-1.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
          </div>

          <div className="relative min-w-0 flex-1 sm:flex-none">
            <label className="sr-only" htmlFor="retention">
              Retention
            </label>
            <select
              id="retention"
              value={retentionMinutes}
              onChange={(event) =>
                setRetentionMinutes(Number(event.target.value))
              }
              className="h-11 w-full min-w-0 appearance-none bg-transparent py-0 pl-1 pr-5 font-mono text-base text-text outline-none sm:h-8 sm:w-auto sm:text-xs"
            >
              {retentionOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <ChevronDown
              className="pointer-events-none absolute right-1.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
          </div>

          <button
            type="button"
            role="switch"
            aria-checked={burnAfterReading}
            onClick={() => setBurnAfterReading(!burnAfterReading)}
            title={
              burnAfterReading
                ? "Burn after reading: best-effort deletion after a successful read; concurrent deployments can race"
                : "Burn after reading is off"
            }
            className={`inline-flex size-11 shrink-0 items-center justify-center gap-1.5 font-mono text-xs transition sm:h-8 sm:w-auto sm:px-2 ${
              burnAfterReading
                ? "text-danger"
                : "text-muted-foreground hover:text-text"
            }`}
          >
            <Flame className="h-4 w-4 sm:h-3.5 sm:w-3.5" aria-hidden="true" />
            <span className="hidden sm:inline">burn</span>
          </button>

          <div className="relative shrink-0">
            <button
              type="button"
              onClick={() => setEncryptionOpen((open) => !open)}
              aria-expanded={isEncryptionOpen}
              aria-haspopup="dialog"
              aria-label="Encryption options"
              title="Encryption options"
              className={`inline-flex size-11 items-center justify-center gap-1.5 font-mono text-xs transition sm:h-8 sm:w-auto sm:px-2 ${
                requiresKey
                  ? "text-text"
                  : "text-muted-foreground hover:text-text"
              }`}
            >
              <Lock className="h-4 w-4 sm:h-3.5 sm:w-3.5" aria-hidden="true" />
              <span className="hidden sm:inline">
                {requiresKey ? encryptionChipLabel[encryption] : "lock"}
              </span>
            </button>
            {isEncryptionOpen && (
              <>
                <button
                  type="button"
                  aria-label="Dismiss encryption"
                  className="fixed inset-0 z-10 bg-background/70 sm:bg-transparent"
                  onClick={() => setEncryptionOpen(false)}
                />
                <div
                  role="dialog"
                  aria-label="Encryption settings"
                  className="fixed inset-x-0 bottom-0 z-20 space-y-4 border-t border-border bg-background p-5 pb-[max(1.25rem,env(safe-area-inset-bottom))] sm:absolute sm:inset-auto sm:bottom-full sm:right-0 sm:mb-1 sm:w-[min(20rem,calc(100vw-2rem))] sm:border sm:p-4 sm:pb-4"
                >
                  <p className="text-xs text-muted-foreground">
                    Encryption runs on the server. Keys transit over TLS, are
                    used in memory, and are not stored.
                  </p>
                  <div className="space-y-1.5">
                    <label className={fieldLabelClasses} htmlFor="encryption">
                      algorithm
                    </label>
                    <select
                      id="encryption"
                      value={encryption}
                      onChange={(event) =>
                        setEncryption(
                          event.target.value as EncryptionAlgorithm,
                        )
                      }
                      className={inputClasses}
                    >
                      {encryptionOptions.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="space-y-1.5">
                    <label
                      className={fieldLabelClasses}
                      htmlFor="encryptionKey"
                    >
                      encryption key
                    </label>
                    <div className="flex gap-2">
                      <input
                        id="encryptionKey"
                        type="password"
                        autoComplete="new-password"
                        value={encryptionKey}
                        onChange={(event) =>
                          setEncryptionKey(event.target.value)
                        }
                        disabled={!requiresKey}
                        placeholder={
                          requiresKey
                            ? "Shared secret or passphrase"
                            : "Enable encryption to set a key"
                        }
                        className={`${inputClasses} min-h-11 font-mono sm:min-h-9`}
                        required={requiresKey}
                      />
                      <button
                        type="button"
                        onClick={handleGenerateEncryptionKey}
                        className="inline-flex h-11 flex-shrink-0 items-center border border-border px-2.5 text-xs font-medium text-text transition hover:bg-muted sm:h-9"
                      >
                        Generate
                      </button>
                    </div>
                  </div>
                </div>
              </>
            )}
          </div>

          <button
            type="submit"
            className="ml-auto hidden h-8 items-center gap-1.5 bg-accent px-4 text-xs font-medium text-accent-foreground transition hover:opacity-80 disabled:opacity-60 sm:inline-flex"
            disabled={mutation.isPending}
            title="Create paste (⌘⏎)"
          >
            {mutation.isPending ? (
              <>
                <span
                  className="h-3 w-3 animate-spin rounded-full border border-current border-t-transparent"
                  aria-hidden="true"
                />
                Creating…
              </>
            ) : (
              <>
                Create
                <kbd
                  className="font-mono text-[10px] opacity-70"
                  aria-hidden="true"
                >
                  ⌘⏎
                </kbd>
              </>
            )}
          </button>
        </div>

        <div className="flex px-3 pt-1 sm:hidden">
          <button
            type="submit"
            className="inline-flex h-12 flex-1 items-center justify-center bg-accent text-base font-medium text-accent-foreground transition hover:opacity-80 disabled:opacity-60"
            disabled={mutation.isPending}
            title="Create paste (⌘⏎)"
          >
            {mutation.isPending ? "Creating…" : "Create"}
          </button>
        </div>
      </div>
    </form>
  );
};

import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { useLocation } from "react-router-dom";
import { useMutation } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  Bot,
  Check,
  ChevronDown,
  Copy,
  Flame,
  ImageDown,
  Lock,
  QrCode,
  ScrollText,
  Share2,
} from "lucide-react";

import { ApiError, createPaste } from "../api/client";
import type { CreatePastePayload, EncryptionAlgorithm, PasteFormat } from "../api/types";
import { MonacoEditor } from "../components/editor/MonacoEditor";
import { OpenWithAgents } from "../components/OpenWithAgents";
import { useHotkeys } from "../hooks/useHotkeys";
import {
  buildPasteShareUrl,
  generateEncryptionKey,
  validateEncryptionKey,
} from "../lib/pasteSecurity";
import {
  downloadBlob,
  pasteIdFromShareUrl,
  renderShareImage,
  shareImageColorsFromDocument,
} from "../lib/shareImage";
import { whisperNote, sharePayload } from "../lib/whisper";
import { agentReceipt } from "../lib/agent";
import { sniffFormatFromText } from "../lib/sniffFormat";
import { MAX_PASTE_BYTES, composerStats, isTextFile, sniffFormat } from "../lib/composer";
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

const encryptionOptions: Array<{ label: string; value: EncryptionAlgorithm }> = [
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
  { label: "1 minute", value: 1 },
  { label: "10 minutes", value: 10 },
  { label: "1 hour", value: 60 },
  { label: "3 hours", value: 180 },
  { label: "1 day", value: 1440 },
  { label: "7 days", value: 10080 },
  { label: "30 days", value: 43200 },
];

const fieldLabelClasses = "block text-xs font-medium text-muted-foreground";

const inputClasses =
  "w-full rounded-md border-0 bg-surface px-3 py-2 text-sm text-text shadow-soft placeholder:text-muted-foreground focus:outline-none focus:shadow-strong disabled:cursor-not-allowed disabled:opacity-50";

// State passed via `navigate("/", { state })` when forking an existing paste.
interface ForkState {
  content?: unknown;
  format?: unknown;
}

const isPasteFormat = (value: unknown): value is PasteFormat =>
  typeof value === "string" && formatOptions.some((option) => option.value === value);

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
  const formatLocked = useRef(isPasteFormat((location.state as ForkState | null)?.format));
  const [autoFormat, setAutoFormat] = useState(!formatLocked.current);
  const [retentionMinutes, setRetentionMinutes] = useState<number>(1440);
  const [encryption, setEncryption] = useState<EncryptionAlgorithm>("none");
  const [encryptionKey, setEncryptionKey] = useState("");
  const [writeCredential, setWriteCredential] = useState(() => {
    if (typeof window === "undefined") return "";
    return sessionStorage.getItem("copypaste.write-token") ?? "";
  });
  const [burnAfterReading, setBurnAfterReading] = useState(false);
  const [shareUrl, setShareUrl] = useState<string | null>(null);
  const [isCopying, setIsCopying] = useState(false);
  const [isSavingImage, setIsSavingImage] = useState(false);
  const [showQr, setShowQr] = useState(false);
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [pasteEncryption, setPasteEncryption] = useState<EncryptionAlgorithm>("none");
  const [pasteEncryptionKey, setPasteEncryptionKey] = useState("");
  const [showWriteToken, setShowWriteToken] = useState(false);
  const [optionsOpen, setOptionsOpen] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef(content);
  const idleRef = useRef(0);
  const [isDragging, setIsDragging] = useState(false);

  const setLiveContent = (next: string) => {
    contentRef.current = next;
    if (typeof window !== "undefined" && window.matchMedia("(max-width: 768px)").matches) {
      window.clearTimeout(idleRef.current);
      idleRef.current = window.setTimeout(() => setContent(next), 200);
      return;
    }
    setContent(next);
  };

  useEffect(() => {
    if (formatLocked.current) return;
    const sniffed = sniffFormatFromText(content);
    if (sniffed && sniffed !== format) {
      setFormat(sniffed);
      setAutoFormat(true);
    }
  }, [content, format]);

  const stats = composerStats(content);

  const applyText = (next: string, filename?: string) => {
    if (new TextEncoder().encode(next).byteLength > MAX_PASTE_BYTES) {
      toast.error("That file is over 1 MB");
      return;
    }
    contentRef.current = next;
    setContent(next);
    if (filename) {
      const sniffed = sniffFormat(filename);
      if (sniffed) {
        formatLocked.current = true;
        setAutoFormat(false);
        setFormat(sniffed);
      }
    }
  };

  const loadFile = async (file: File) => {
    if (!isTextFile(file)) {
      toast.error("Drop a text file");
      return;
    }
    if (file.size > MAX_PASTE_BYTES) {
      toast.error("That file is over 1 MB");
      return;
    }
    applyText(await file.text(), file.name);
  };

  const handlePasteClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (!text.trim()) {
        toast.error("Clipboard is empty");
        return;
      }
      applyText(text);
      toast.success("Pasted");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Unable to read clipboard", { description: message });
    }
  };

  const mutation = useMutation({
    mutationFn: async () => {
      const live = contentRef.current;
      const payload: CreatePastePayload = {
        content: live,
        format,
        retention_minutes: retentionMinutes ? Number(retentionMinutes) : undefined,
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
      try {
        navigator.vibrate?.(16);
      } catch {
        /* ignore */
      }
      setPasteEncryption(usedEncryption);
      setPasteEncryptionKey(usedEncryptionKey);
      setContent("");
      contentRef.current = "";
      setShareUrl(result.shareableUrl);
      setEncryptionKey("");
      if (usedEncryption !== "none") {
        setEncryption("none");
      }
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : "Unknown error";
      if (error instanceof ApiError && error.status === 401) {
        setShowWriteToken(true);
        toast.error("Write token required", { description: message });
        return;
      }
      toast.error("Failed to create paste", { description: message });
    },
  });

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const live = contentRef.current;
    if (!live.trim()) {
      document.getElementById("content")?.focus();
      return;
    }
    if (encryption !== "none") {
      const encryptionKeyError = validateEncryptionKey(encryptionKey);
      if (encryptionKeyError) {
        toast.error("Invalid encryption key", {
          description: encryptionKeyError,
        });
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

  const toggleEncryption = () => {
    if (encryption === "none") {
      setEncryption("aes256_gcm");
      if (!encryptionKey) {
        const key = createSecureEncryptionKey();
        if (key) setEncryptionKey(key);
      }
      return;
    }
    setEncryption("none");
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

  const handleCopyWhisper = async () => {
    const urlToCopy = shareLink;
    if (!urlToCopy) return;
    try {
      setIsCopying(true);
      await navigator.clipboard.writeText(whisperNote(urlToCopy));
      toast.success("Share note copied");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Unable to copy note", { description: message });
    } finally {
      setIsCopying(false);
    }
  };

  const handleCopyAgent = async () => {
    const urlToCopy = shareLink;
    if (!urlToCopy) return;
    try {
      setIsCopying(true);
      const key = pasteEncryption !== "none" && pasteEncryptionKey ? pasteEncryptionKey : undefined;
      await navigator.clipboard.writeText(
        agentReceipt(urlToCopy, key, pasteEncryption !== "none" ? pasteEncryption : undefined),
      );
      toast.success("Agent receipt copied");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Unable to copy agent receipt", { description: message });
    } finally {
      setIsCopying(false);
    }
  };

  const handleShareLink = async () => {
    const urlToShare = shareLink;
    if (!urlToShare) return;
    if (typeof navigator.share === "function") {
      try {
        await navigator.share(sharePayload(urlToShare));
      } catch (error) {
        // The user dismissing the share sheet is not an error worth surfacing.
        if (error instanceof Error && error.name === "AbortError") return;
        const message = error instanceof Error ? error.message : "Unknown error";
        toast.error("Unable to share link", { description: message });
      }
      return;
    }
    await handleCopyShareUrl();
  };

  const handleSaveImage = async () => {
    const urlToShare = shareLink;
    if (!urlToShare) return;
    try {
      setIsSavingImage(true);
      const blob = await renderShareImage(urlToShare, shareImageColorsFromDocument(), {
        burn: burnAfterReading,
        encryptionLabel:
          pasteEncryption !== "none" ? encryptionChipLabel[pasteEncryption] : undefined,
      });
      const file = new File([blob], `copypaste-${pasteIdFromShareUrl(urlToShare)}.png`, {
        type: "image/png",
      });
      const canShareFile =
        typeof navigator.share === "function" &&
        typeof navigator.canShare === "function" &&
        navigator.canShare({ files: [file] });
      if (canShareFile) {
        try {
          await navigator.share({
            files: [file],
            title: "copypaste.fyi",
          });
          return;
        } catch (error) {
          if (error instanceof Error && error.name === "AbortError") return;
        }
      }
      downloadBlob(file, file.name);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Unable to make share image", { description: message });
    } finally {
      setIsSavingImage(false);
    }
  };

  useEffect(() => {
    if (requiresKey) setOptionsOpen(true);
  }, [requiresKey]);

  // Render the QR code lazily — only once the toggle is opened.
  useEffect(() => {
    if (!shareLink || !showQr) {
      setQrDataUrl(null);
      return;
    }
    let cancelled = false;
    void import("qrcode")
      .then((QRCode) => QRCode.toDataURL(shareLink, { margin: 1, width: 160 }))
      .then((dataUrl) => {
        if (!cancelled) setQrDataUrl(dataUrl);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setQrDataUrl(null);
        const message = error instanceof Error ? error.message : "Unknown error";
        toast.error("Unable to generate QR code", { description: message });
      });
    return () => {
      cancelled = true;
    };
  }, [shareLink, showQr]);

  return (
    <form
      ref={formRef}
      className="flex min-h-0 flex-1 flex-col overflow-hidden"
      onSubmit={handleSubmit}
    >
      {shareLink ? (
        <section
          className="flex min-h-0 flex-1 flex-col items-center justify-center px-4 py-6 sm:px-5 sm:py-10"
          aria-label="Paste created"
        >
          <div className="w-full max-w-md space-y-6">
            <div className="space-y-2">
              <p className="inline-flex items-center gap-2 text-sm font-medium text-success">
                <Check className="h-4 w-4" aria-hidden="true" />
                Live
              </p>
              <h2 className="text-2xl font-medium tracking-tight text-text">
                Paste is ready to share
              </h2>
              <p className="text-sm leading-relaxed text-muted-foreground">
                Anyone with this link can open it. There is no public listing.
              </p>
            </div>
            <div className="flex flex-col gap-2 sm:flex-row">
              <input
                id="share-url"
                readOnly
                value={shareLink}
                onFocus={(event) => event.target.select()}
                className={`${inputClasses} min-h-12 font-mono text-xs sm:min-h-11`}
              />
              <button
                type="button"
                onClick={handleCopyShareUrl}
                disabled={isCopying}
                className="inline-flex h-12 shrink-0 items-center justify-center gap-2 rounded-md bg-accent px-4 text-sm font-medium text-accent-foreground transition hover:opacity-90 disabled:opacity-60 sm:h-11 sm:w-auto"
                aria-label={isCopying ? "Copying link…" : "Copy link"}
              >
                <Copy className="h-4 w-4" aria-hidden="true" />
                {isCopying ? "Copying" : "Copy"}
              </button>
            </div>
            <div className="grid grid-cols-2 gap-2 sm:flex sm:flex-wrap">
              <button
                type="button"
                onClick={handleShareLink}
                className="inline-flex h-12 items-center justify-center gap-2 rounded-md bg-muted px-3 text-sm text-text sm:h-11"
                aria-label="Share link"
              >
                <Share2 className="h-4 w-4" aria-hidden="true" />
                Share
              </button>
              <button
                type="button"
                onClick={() => setShowQr((open) => !open)}
                aria-pressed={showQr}
                className={`inline-flex h-12 items-center justify-center gap-2 rounded-md px-3 text-sm sm:h-11 ${
                  showQr ? "bg-text text-background" : "bg-muted text-text"
                }`}
                aria-label={showQr ? "Hide QR code" : "Show QR code"}
              >
                <QrCode className="h-4 w-4" aria-hidden="true" />
                QR
              </button>
              <button
                type="button"
                onClick={() => void handleSaveImage()}
                disabled={isSavingImage}
                className="inline-flex h-12 items-center justify-center gap-2 rounded-md bg-muted px-3 text-sm text-text disabled:opacity-60 sm:h-11"
                aria-label={isSavingImage ? "Saving share image…" : "Save share image"}
              >
                <ImageDown className="h-4 w-4" aria-hidden="true" />
                {isSavingImage ? "Saving" : "Image"}
              </button>
              <button
                type="button"
                onClick={() => void handleCopyWhisper()}
                disabled={isCopying}
                className="inline-flex h-12 items-center justify-center gap-2 rounded-md bg-muted px-3 text-sm text-text disabled:opacity-60 sm:h-11"
                aria-label="Copy a share note"
              >
                <ScrollText className="h-4 w-4" aria-hidden="true" />
                Note
              </button>
              <button
                type="button"
                onClick={() => void handleCopyAgent()}
                disabled={isCopying}
                className="col-span-2 inline-flex h-12 items-center justify-center gap-2 rounded-md bg-muted px-3 text-sm text-text disabled:opacity-60 sm:col-auto sm:h-11"
                aria-label="Copy agent receipt"
              >
                <Bot className="h-4 w-4" aria-hidden="true" />
                Agent
              </button>
            </div>
            <OpenWithAgents url={shareLink} />
            {showQr && qrDataUrl && (
              <div className="w-fit rounded-lg bg-surface p-2 shadow-soft">
                <img
                  src={qrDataUrl}
                  alt="QR code for the paste share link"
                  width={160}
                  height={160}
                  className="block h-40 w-40 rounded-md"
                />
              </div>
            )}
            {pasteEncryption !== "none" && pasteEncryptionKey && (
              <div className="space-y-1.5">
                <label className={fieldLabelClasses} htmlFor="share-key">
                  Encryption key — share out of band
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
            <div className="flex flex-col gap-2 sm:flex-row">
              <button
                type="button"
                onClick={() => {
                  setShareUrl(null);
                  setShowQr(false);
                }}
                className="inline-flex h-12 flex-1 items-center justify-center rounded-md bg-muted text-sm font-medium sm:h-11"
              >
                New paste
              </button>
              <a
                href={shareLink}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex h-12 flex-1 items-center justify-center rounded-md text-sm font-medium text-muted-foreground hover:bg-muted hover:text-text sm:h-11"
              >
                Open
              </a>
            </div>
          </div>
        </section>
      ) : (
        <>
          <label className="sr-only" htmlFor="content">
            Content
          </label>
          <div
            className={`relative min-h-0 flex-1 overflow-hidden ${isDragging ? "bg-muted/40" : ""}`}
            onDragEnter={(event) => {
              event.preventDefault();
              setIsDragging(true);
            }}
            onDragOver={(event) => {
              event.preventDefault();
              setIsDragging(true);
            }}
            onDragLeave={() => setIsDragging(false)}
            onDrop={(event) => {
              event.preventDefault();
              setIsDragging(false);
              const file = event.dataTransfer.files[0];
              if (file) void loadFile(file);
            }}
          >
            <MonacoEditor
              value={content}
              onChange={setLiveContent}
              format={format}
              height="100%"
              className="absolute inset-0 h-full min-h-0 w-full"
            />
            {isDragging ? (
              <p className="pointer-events-none absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
                Drop to replace the box
              </p>
            ) : null}
          </div>
        </>
      )}
      {!shareLink && (
        <div className="shrink-0 border-t border-border bg-surface pb-[max(0.75rem,calc(var(--keyboard-inset,0px)+env(safe-area-inset-bottom)))]">
          <div className="flex items-center gap-2 px-3 pt-2 text-xs text-muted-foreground sm:px-4">
            <span>
              {stats.chars === 0
                ? "Empty"
                : `${stats.chars} chars · ${stats.lines} line${stats.lines === 1 ? "" : "s"}`}
            </span>
            {stats.bytes > MAX_PASTE_BYTES * 0.9 ? (
              <span className="text-danger">{Math.ceil(stats.bytes / 1024)} KB / 1024 KB</span>
            ) : null}
            <span className="ml-auto flex items-center gap-2">
              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                className="text-text underline-offset-2 hover:underline"
              >
                Open file
              </button>
              <button
                type="button"
                onClick={() => void handlePasteClipboard()}
                className="text-text underline-offset-2 hover:underline"
              >
                Paste
              </button>
            </span>
            <input
              ref={fileInputRef}
              type="file"
              accept="text/*,.md,.json,.js,.ts,.py,.rs,.go,.rb,.sh,.yml,.yaml,.sql,.html,.css,.txt"
              className="hidden"
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = "";
                if (file) void loadFile(file);
              }}
            />
          </div>
          {(showWriteToken || writeCredential) && (
            <div className="space-y-1.5 px-3 pt-3 sm:px-4">
              <label className={fieldLabelClasses} htmlFor="write-credential">
                Write token
              </label>
              <input
                id="write-credential"
                type="password"
                autoComplete="off"
                value={writeCredential}
                onChange={(event) => {
                  const next = event.target.value;
                  setWriteCredential(next);
                  sessionStorage.setItem("copypaste.write-token", next);
                }}
                placeholder="Operator credential for this instance"
                className={`${inputClasses} min-h-12 sm:min-h-10`}
              />
              <p className="text-xs leading-relaxed text-muted-foreground">
                Sent as X-CopyPaste-Write-Token. Stored only in this browser session.
              </p>
            </div>
          )}
          {requiresKey && (
            <div className="space-y-2 px-3 pt-3 sm:px-4">
              <div className="grid gap-2 sm:grid-cols-[minmax(0,11rem)_minmax(0,1fr)_auto] sm:items-end">
                <div className="min-w-0">
                  <label className={fieldLabelClasses} htmlFor="encryption">
                    Algorithm
                  </label>
                  <select
                    id="encryption"
                    value={encryption}
                    onChange={(event) => setEncryption(event.target.value as EncryptionAlgorithm)}
                    className={`${inputClasses} min-h-12 sm:min-h-10`}
                  >
                    {encryptionOptions
                      .filter((option) => option.value !== "none")
                      .map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                  </select>
                </div>
                <div className="min-w-0">
                  <label className={fieldLabelClasses} htmlFor="encryptionKey">
                    Encryption key
                  </label>
                  <input
                    id="encryptionKey"
                    type="password"
                    autoComplete="new-password"
                    value={encryptionKey}
                    onChange={(event) => setEncryptionKey(event.target.value)}
                    placeholder="Shared secret or passphrase"
                    className={`${inputClasses} min-h-12 font-mono sm:min-h-10`}
                    required
                  />
                </div>
                <button
                  type="button"
                  onClick={handleGenerateEncryptionKey}
                  className="inline-flex h-12 w-full shrink-0 items-center justify-center rounded-md bg-muted px-3 text-sm font-medium text-text hover:bg-border sm:h-10 sm:w-auto"
                >
                  Generate
                </button>
              </div>
              <p className="text-xs leading-relaxed text-muted-foreground">
                Encryption runs on the server. Keys transit over TLS and are not stored.
              </p>
            </div>
          )}
          <div className="px-3 pt-3 sm:hidden">
            <button
              type="button"
              aria-expanded={optionsOpen}
              onClick={() => setOptionsOpen((value) => !value)}
              className="flex h-11 w-full items-center justify-between rounded-md bg-muted px-3 text-left text-sm text-text transition active:scale-[0.96]"
            >
              <span className="min-w-0 truncate">
                {[
                  formatOptions.find((item) => item.value === format)?.label,
                  retentionOptions.find((item) => item.value === retentionMinutes)?.label,
                  burnAfterReading ? "Burn" : null,
                  requiresKey ? encryptionChipLabel[encryption] : null,
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
              <ChevronDown
                className={`h-4 w-4 shrink-0 text-muted-foreground transition-transform ${
                  optionsOpen ? "rotate-180" : ""
                }`}
              />
            </button>
          </div>
          <div className="flex flex-col gap-2 px-3 pt-2 sm:flex-row sm:items-end sm:gap-3 sm:px-4 sm:pt-3">
            <div
              className={`grid grid-cols-2 gap-2 sm:flex sm:min-w-0 sm:flex-1 sm:items-end ${
                optionsOpen ? "" : "max-sm:hidden"
              }`}
            >
              <DockSelect
                id="format"
                label={autoFormat ? "Format · auto" : "Format"}
                value={format}
                onChange={(value) => {
                  formatLocked.current = true;
                  setAutoFormat(false);
                  setFormat(value as PasteFormat);
                }}
              >
                {formatOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </DockSelect>

              <DockSelect
                id="retention"
                label="Retention period"
                value={String(retentionMinutes)}
                onChange={(value) => setRetentionMinutes(Number(value))}
              >
                {retentionOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </DockSelect>

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
                className={`inline-flex h-11 w-full items-center justify-center gap-2 self-end rounded-md px-3 text-sm sm:h-10 sm:w-auto ${
                  burnAfterReading
                    ? "bg-muted text-danger"
                    : "bg-muted text-muted-foreground hover:text-text"
                }`}
              >
                <Flame className="h-3.5 w-3.5" aria-hidden="true" />
                Burn
              </button>

              <button
                type="button"
                onClick={toggleEncryption}
                aria-pressed={requiresKey}
                aria-label="Encryption options"
                title="Encryption options"
                className={`inline-flex h-11 w-full items-center justify-center gap-2 self-end rounded-md px-3 text-sm sm:h-10 sm:w-auto ${
                  requiresKey
                    ? "bg-accent text-accent-foreground"
                    : "bg-muted text-muted-foreground hover:text-text"
                }`}
              >
                <Lock className="h-3.5 w-3.5" aria-hidden="true" />
                {requiresKey ? encryptionChipLabel[encryption] : "Encrypt"}
              </button>
            </div>

            <button
              type="submit"
              className="hidden h-10 items-center justify-center rounded-lg bg-accent px-5 text-sm font-medium text-accent-foreground transition hover:opacity-90 disabled:opacity-60 sm:inline-flex sm:w-auto sm:flex-none"
              disabled={mutation.isPending}
              title="Get link (⌘⏎)"
            >
              {mutation.isPending ? "Creating…" : "Get link"}
              {!mutation.isPending && (
                <kbd className="ml-2 font-mono text-[10px] opacity-70" aria-hidden="true">
                  ⌘⏎
                </kbd>
              )}
            </button>
          </div>
          <div className="px-3 pt-1 sm:hidden">
            <button
              type="submit"
              className="inline-flex h-12 w-full items-center justify-center rounded-lg bg-accent px-5 text-base font-medium text-accent-foreground transition hover:opacity-90 disabled:opacity-60"
              disabled={mutation.isPending}
            >
              {mutation.isPending ? "Creating…" : "Get link"}
            </button>
          </div>
        </div>
      )}
    </form>
  );
};

function DockSelect({
  id,
  label,
  value,
  onChange,
  children,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  children: React.ReactNode;
}) {
  return (
    <div className="min-w-0">
      <label
        className="mb-1 block text-xs font-medium text-muted-foreground sm:sr-only"
        htmlFor={id}
      >
        {label}
      </label>
      <div className="relative">
        <select
          id={id}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          className="h-11 w-full appearance-none rounded-md bg-muted px-3 pr-8 text-sm text-text outline-none sm:h-10 sm:w-auto sm:min-w-36"
        >
          {children}
        </select>
        <ChevronDown
          className="pointer-events-none absolute right-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground"
          aria-hidden="true"
        />
      </div>
    </div>
  );
}

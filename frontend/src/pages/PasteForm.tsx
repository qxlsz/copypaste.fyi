import { useEffect, useMemo, useRef, useState } from "react";
import type { ChangeEvent, FormEvent } from "react";
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
  StegoRequest,
} from "../api/types";
import { MonacoEditor } from "../components/editor/MonacoEditor";
import { useHotkeys } from "../hooks/useHotkeys";
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

const BUILTIN_STEGO_CARRIERS: Array<{
  id: string;
  name: string;
  description: string;
}> = [
  {
    id: "aurora",
    name: "Aurora",
    description: "Cool gradients with soft lighting.",
  },
  {
    id: "horizon",
    name: "Horizon",
    description: "Sunset-inspired blues and ambers.",
  },
  { id: "prism", name: "Prism", description: "Abstract neon waves (default)." },
  {
    id: "nebula",
    name: "Nebula",
    description: "Cosmic purples flecked with speckled stardust.",
  },
  {
    id: "solstice",
    name: "Solstice",
    description: "Warm sunrise oranges fading into sky blues.",
  },
  {
    id: "midnight",
    name: "Midnight",
    description: "City-lights palette with cool blues and amber sparks.",
  },
  {
    id: "cinder",
    name: "Cinder",
    description: "Charcoal base with ember highlights for high contrast.",
  },
];

const PASS_ADJECTIVES = [
  "stellar",
  "quantum",
  "radiant",
  "luminous",
  "hyper",
  "galactic",
  "neon",
  "cosmic",
  "orbital",
  "sonic",
];
const PASS_NOUNS = [
  "otter",
  "phoenix",
  "nebula",
  "flux",
  "cipher",
  "tachyon",
  "comet",
  "formula",
  "byte",
  "matrix",
];
const PASS_SUFFIXES = [
  "42",
  "9000",
  "1337",
  "7g",
  "mk2",
  "ix",
  "hyperlane",
  "vortex",
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
  const { user } = useAuth();
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
  const [retentionMinutes, setRetentionMinutes] = useState<number>(0);
  const [encryption, setEncryption] = useState<EncryptionAlgorithm>("none");
  const [encryptionKey, setEncryptionKey] = useState("");
  const [burnAfterReading, setBurnAfterReading] = useState(false);
  const [shareUrl, setShareUrl] = useState<string | null>(null);
  const [isCopying, setIsCopying] = useState(false);
  const [showQr, setShowQr] = useState(false);
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [pasteEncryption, setPasteEncryption] =
    useState<EncryptionAlgorithm>("none");
  const [pasteEncryptionKey, setPasteEncryptionKey] = useState("");
  const [useStego, setUseStego] = useState(false);
  const [stegoMode, setStegoMode] = useState<"builtin" | "uploaded">("builtin");
  const [stegoCarrierId, setStegoCarrierId] = useState("prism");
  const [stegoUploadName, setStegoUploadName] = useState<string | null>(null);
  const [stegoUploadData, setStegoUploadData] = useState<string | null>(null);
  const [stegoError, setStegoError] = useState<string | null>(null);
  const [isEncryptionOpen, setEncryptionOpen] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);

  const handleStegoFileUpload = async (
    event: ChangeEvent<HTMLInputElement>,
  ) => {
    const file = event.target.files?.[0];
    if (!file) {
      setStegoUploadData(null);
      setStegoUploadName(null);
      return;
    }
    if (!file.type.startsWith("image/")) {
      setStegoError("Please choose an image file for carrier embedding.");
      setStegoUploadData(null);
      setStegoUploadName(null);
      return;
    }
    try {
      const arrayBuffer = await file.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);
      const base64 = btoa(String.fromCharCode(...bytes));
      const dataUri = `data:${file.type};base64,${base64}`;
      setStegoUploadData(dataUri);
      setStegoUploadName(file.name);
      setStegoError(null);
    } catch (error) {
      console.error(error);
      setStegoError(
        "Failed to read file. Please try again or pick a different image.",
      );
      setStegoUploadData(null);
      setStegoUploadName(null);
    }
  };

  const mutation = useMutation({
    mutationFn: async () => {
      const payload: CreatePastePayload = {
        content,
        format,
        retention_minutes: retentionMinutes
          ? Number(retentionMinutes)
          : undefined,
        burn_after_reading: burnAfterReading || undefined,
        owner_pubkey_hash: user?.pubkeyHash,
      };

      if (encryption !== "none") {
        payload.encryption = {
          algorithm: encryption,
          key: encryptionKey,
        };
      }

      if (useStego && encryption !== "none") {
        let stegoPayload: StegoRequest | undefined;
        if (stegoMode === "builtin") {
          stegoPayload = { mode: "builtin", carrier: stegoCarrierId };
        } else if (stegoMode === "uploaded" && stegoUploadData) {
          stegoPayload = { mode: "uploaded", data_uri: stegoUploadData };
        }

        if (stegoPayload) {
          payload.stego = stegoPayload;
        }
      }

      return createPaste(payload);
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

  useEffect(() => {
    if (!requiresKey) {
      setUseStego(false);
    }
  }, [requiresKey]);

  const buildRandomPassphrase = () => {
    const randomElement = <T,>(items: T[]) =>
      items[Math.floor(Math.random() * items.length)];
    return `${randomElement(PASS_ADJECTIVES)}-${randomElement(PASS_NOUNS)}-${randomElement(PASS_SUFFIXES)}`;
  };

  const generatePassphrase = () => {
    const phrase = buildRandomPassphrase();
    setEncryptionKey(phrase);
    if (encryption === "none") {
      setEncryption("aes256_gcm");
    }
    toast.message("Geeky passphrase generated", { description: phrase });
  };

  const shareLink = useMemo(() => {
    if (!shareUrl) {
      return null;
    }

    try {
      const path = `/p${shareUrl}`;
      const url = new URL(path, window.location.origin);
      if (retentionMinutes && retentionMinutes > 0) {
        url.searchParams.set("ttl", retentionMinutes.toString());
      }
      // Keep the key in the URL fragment so it never reaches server logs,
      // browser history sync, or Referer headers.
      if (pasteEncryption !== "none" && pasteEncryptionKey.trim()) {
        url.hash = `key=${encodeURIComponent(pasteEncryptionKey)}`;
      }
      return url.toString();
    } catch {
      return `/p${shareUrl}`;
    }
  }, [shareUrl, pasteEncryption, pasteEncryptionKey, retentionMinutes]);

  const handleCopyShareUrl = async () => {
    const urlToCopy = shareLink || shareUrl;
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
    const urlToShare = shareLink || shareUrl;
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
    <form ref={formRef} className="space-y-4" onSubmit={handleSubmit}>
      {shareLink && (
        <section
          className="space-y-3 rounded-lg border border-border bg-surface p-4"
          aria-label="Paste created"
        >
          <div className="flex items-center gap-2">
            <Check className="h-4 w-4 text-success" aria-hidden="true" />
            <h2 className="text-sm font-semibold tracking-tight text-text">
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
                className={`${inputClasses} font-mono text-xs`}
              />
              <button
                type="button"
                onClick={handleCopyShareUrl}
                disabled={isCopying}
                className="inline-flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md border border-border text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface disabled:opacity-60"
                aria-label={isCopying ? "Copying link…" : "Copy link"}
                title="Copy link"
              >
                <Copy className="h-4 w-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                onClick={handleShareLink}
                className="inline-flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md border border-border text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface"
                aria-label="Share link"
                title="Share link"
              >
                <Share2 className="h-4 w-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                onClick={() => setShowQr((open) => !open)}
                aria-pressed={showQr}
                className={`inline-flex h-9 flex-shrink-0 items-center gap-1.5 rounded-md border border-border px-2.5 font-mono text-[11px] transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface ${
                  showQr
                    ? "bg-accent/10 text-accent"
                    : "text-muted-foreground hover:bg-muted hover:text-text"
                }`}
                aria-label={showQr ? "Hide QR code" : "Show QR code"}
                title={showQr ? "Hide QR code" : "Show QR code"}
              >
                <QrCode className="h-4 w-4" aria-hidden="true" />
                qr
              </button>
            </div>
            {showQr && qrDataUrl && (
              <div className="w-fit rounded-md border border-border bg-surface p-2">
                <img
                  src={qrDataUrl}
                  alt="QR code for the paste share link"
                  width={160}
                  height={160}
                  className="block h-40 w-40 rounded-sm"
                />
              </div>
            )}
            <a
              href={shareLink}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-block text-xs text-accent underline-offset-2 hover:underline"
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

      <section className="overflow-visible rounded-lg border border-border bg-surface">
        {/* Toolbar */}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-border px-3 py-2">
          <div className="relative">
            <label className="sr-only" htmlFor="format">
              Format
            </label>
            <select
              id="format"
              value={format}
              onChange={(event) => setFormat(event.target.value as PasteFormat)}
              className="appearance-none rounded-md border-0 bg-transparent py-1 pl-2 pr-7 font-mono text-xs text-text transition hover:bg-muted focus:outline-none focus:ring-1 focus:ring-accent"
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

          <div
            className="flex items-center overflow-hidden rounded-md border border-border"
            role="group"
            aria-label="Retention period"
          >
            {retentionOptions.map((opt, index) => (
              <button
                key={opt.value}
                type="button"
                onClick={() => setRetentionMinutes(opt.value)}
                aria-pressed={retentionMinutes === opt.value}
                className={`px-2 py-1 font-mono text-[11px] transition focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-accent ${
                  index > 0 ? "border-l border-border" : ""
                } ${
                  retentionMinutes === opt.value
                    ? "bg-accent/10 font-semibold text-accent"
                    : "text-muted-foreground hover:bg-muted hover:text-text"
                }`}
              >
                {opt.label}
              </button>
            ))}
          </div>

          <button
            type="button"
            role="switch"
            aria-checked={burnAfterReading}
            onClick={() => setBurnAfterReading(!burnAfterReading)}
            title={
              burnAfterReading
                ? "Burn after reading: paste disappears after first view"
                : "Burn after reading is off"
            }
            className={`inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface ${
              burnAfterReading
                ? "bg-danger/10 font-medium text-danger"
                : "text-muted-foreground hover:bg-muted hover:text-text"
            }`}
          >
            <Flame className="h-3.5 w-3.5" aria-hidden="true" />
            burn
          </button>

          <div className="ml-auto flex items-center gap-2">
            {requiresKey && (
              <span className="hidden rounded border border-accent/40 px-1.5 py-0.5 font-mono text-[10px] text-accent sm:inline-block">
                {encryptionChipLabel[encryption]}
              </span>
            )}
            <div className="relative">
              <button
                type="button"
                onClick={() => setEncryptionOpen((open) => !open)}
                aria-expanded={isEncryptionOpen}
                aria-haspopup="dialog"
                aria-label="Encryption options"
                title="Encryption options"
                className={`inline-flex h-8 w-8 items-center justify-center rounded-md transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface ${
                  requiresKey
                    ? "bg-accent/10 text-accent"
                    : "text-muted-foreground hover:bg-muted hover:text-text"
                }`}
              >
                <Lock className="h-4 w-4" aria-hidden="true" />
              </button>
              {isEncryptionOpen && (
                <>
                  <div
                    className="fixed inset-0 z-10"
                    aria-hidden="true"
                    onClick={() => setEncryptionOpen(false)}
                  />
                  <div
                    role="dialog"
                    aria-label="Encryption settings"
                    className="absolute right-0 top-full z-20 mt-2 w-[min(20rem,calc(100vw-2rem))] space-y-4 rounded-lg border border-border bg-surface p-4"
                  >
                    <p className="text-xs text-muted-foreground">
                      Keys stay client-side — share them out of band.
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
                          className={`${inputClasses} font-mono`}
                          required={requiresKey}
                        />
                        <button
                          type="button"
                          onClick={generatePassphrase}
                          className="inline-flex flex-shrink-0 items-center rounded-md border border-border px-2.5 text-xs font-medium text-text transition hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface"
                        >
                          Generate
                        </button>
                      </div>
                    </div>

                    <div className="space-y-2 border-t border-border pt-3">
                      <div className="flex items-start justify-between gap-3">
                        <div>
                          <p className="text-xs font-medium text-text">
                            Steganographic cover
                          </p>
                          <p className="text-xs text-muted-foreground">
                            Hide ciphertext inside a carrier image.
                          </p>
                        </div>
                        <label
                          className={`inline-flex items-center gap-1.5 text-xs text-text ${!requiresKey ? "opacity-60" : ""}`}
                        >
                          <input
                            type="checkbox"
                            className="h-3.5 w-3.5 rounded border-border bg-surface text-accent focus:ring-accent"
                            checked={useStego}
                            onChange={(event) => {
                              const checked = event.target.checked;
                              if (checked) {
                                if (encryption === "none") {
                                  const phrase = buildRandomPassphrase();
                                  setEncryption("aes256_gcm");
                                  setEncryptionKey(phrase);
                                  toast.message(
                                    "Encryption enabled for steganography",
                                    {
                                      description: phrase,
                                    },
                                  );
                                } else if (!encryptionKey.trim()) {
                                  const phrase = buildRandomPassphrase();
                                  setEncryptionKey(phrase);
                                  toast.message(
                                    "Encryption key generated for steganography",
                                    {
                                      description: phrase,
                                    },
                                  );
                                }
                              }
                              setUseStego(checked);
                            }}
                          />
                          Enable
                        </label>
                      </div>

                      {useStego ? (
                        <div className="space-y-3">
                          <fieldset className="space-y-1.5">
                            <legend className={fieldLabelClasses}>
                              carrier source
                            </legend>
                            <label className="flex items-center gap-2 text-xs text-text">
                              <input
                                type="radio"
                                name="stego-mode"
                                value="builtin"
                                checked={stegoMode === "builtin"}
                                onChange={() => setStegoMode("builtin")}
                                className="h-3.5 w-3.5 border-border text-accent focus:ring-accent"
                              />
                              Bundled artwork
                            </label>
                            <label className="flex items-center gap-2 text-xs text-text">
                              <input
                                type="radio"
                                name="stego-mode"
                                value="uploaded"
                                checked={stegoMode === "uploaded"}
                                onChange={() => setStegoMode("uploaded")}
                                className="h-3.5 w-3.5 border-border text-accent focus:ring-accent"
                              />
                              Upload my own image
                            </label>
                          </fieldset>

                          {stegoMode === "builtin" ? (
                            <div className="space-y-1.5">
                              <label
                                className={fieldLabelClasses}
                                htmlFor="builtinCarrier"
                              >
                                select carrier
                              </label>
                              <select
                                id="builtinCarrier"
                                value={stegoCarrierId}
                                onChange={(event) =>
                                  setStegoCarrierId(event.target.value)
                                }
                                className={inputClasses}
                              >
                                {BUILTIN_STEGO_CARRIERS.map((carrier) => (
                                  <option key={carrier.id} value={carrier.id}>
                                    {carrier.name} — {carrier.description}
                                  </option>
                                ))}
                              </select>
                            </div>
                          ) : (
                            <div className="space-y-1.5">
                              <label
                                className={fieldLabelClasses}
                                htmlFor="stegoUpload"
                              >
                                upload carrier image (png recommended)
                              </label>
                              <input
                                id="stegoUpload"
                                type="file"
                                accept="image/png,image/bmp"
                                onChange={handleStegoFileUpload}
                                className="block w-full text-xs text-muted-foreground file:mr-3 file:rounded-md file:border file:border-solid file:border-border file:bg-surface file:px-2.5 file:py-1.5 file:text-xs file:font-medium file:text-text hover:file:bg-muted"
                              />
                              <p className="text-xs text-muted-foreground">
                                {stegoUploadName
                                  ? `Selected: ${stegoUploadName}`
                                  : "Lossless formats yield better hiding capacity. 1 MB max."}
                              </p>
                            </div>
                          )}
                          {stegoError ? (
                            <p className="text-xs text-danger">{stegoError}</p>
                          ) : null}
                        </div>
                      ) : (
                        <p className="text-xs text-muted-foreground">
                          {requiresKey
                            ? "Enable steganography to embed the encrypted payload inside a carrier image."
                            : "Turn on encryption to unlock steganographic embedding."}
                        </p>
                      )}
                    </div>
                  </div>
                </>
              )}
            </div>

            <button
              type="submit"
              className="inline-flex h-8 items-center gap-1.5 rounded-md bg-accent px-4 text-xs font-medium text-accent-foreground transition hover:bg-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface disabled:opacity-60"
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
                    className="hidden font-mono text-[10px] opacity-70 sm:inline"
                    aria-hidden="true"
                  >
                    ⌘⏎
                  </kbd>
                </>
              )}
            </button>
          </div>
        </div>

        {/* Editor */}
        <label className="sr-only" htmlFor="content">
          Content
        </label>
        <MonacoEditor
          value={content}
          onChange={setContent}
          format={format}
          height="min(68vh, 52rem)"
          className="min-h-[45vh] w-full md:min-h-[60vh]"
        />
      </section>
    </form>
  );
};

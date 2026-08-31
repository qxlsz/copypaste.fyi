import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Check, ChevronDown, Copy, Eye, EyeOff, Flame, Lock, Share2 } from "lucide-react";
import { toast } from "sonner";
import { CodeEditor } from "@/components/code-editor";
import { useHotkeys } from "@/hooks/use-hotkeys";
import {
  buildShareUrl,
  encryptPaste,
  generateEncryptionKey,
  validateEncryptionKey,
} from "@/lib/crypto-paste";
import {
  byteLength,
  detectFormat,
  formatFromFilename,
  MAX_PASTE_BYTES,
  PASTE_FORMATS,
  RETENTION_OPTIONS,
  type PasteFormat,
  isPasteFormat,
} from "@/lib/formats";
import { DEFAULT_API_TTL_MINUTES } from "@/lib/protocol";
import { createPaste } from "@/lib/pastes";
import { previewFrom, rememberPaste } from "@/lib/recents";
import { cn, copyText, readTextFile } from "@/lib/utils";

const fieldLabel = "block font-mono text-2xs text-muted-foreground";
const inputClass =
  "w-full border border-border bg-background px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground outline-none focus:border-foreground disabled:opacity-50";
const selectChip =
  "h-11 w-full min-w-0 appearance-none bg-transparent py-0 pl-1 pr-5 font-mono text-base text-foreground outline-none sm:h-8 sm:w-auto sm:text-xs";
const primaryBtn =
  "inline-flex items-center justify-center gap-2 bg-foreground font-medium text-background transition-opacity duration-150 hover:opacity-80 active:opacity-70 disabled:opacity-40";
const ghostBtn =
  "inline-flex size-11 shrink-0 items-center justify-center gap-1.5 font-mono text-xs transition-colors duration-150 sm:h-8 sm:w-auto sm:px-2";

type ForkState = { content?: string; format?: string };

function readFork(): ForkState | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = sessionStorage.getItem("copypaste.fork");
    if (!raw) return null;
    sessionStorage.removeItem("copypaste.fork");
    return JSON.parse(raw) as ForkState;
  } catch {
    return null;
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(bytes > 10_240 ? 0 : 1)} KB`;
}

export function Composer() {
  const navigate = useNavigate();
  const formRef = useRef<HTMLFormElement>(null);
  const fork = useRef(readFork());
  const [content, setContent] = useState(fork.current?.content ?? "");
  const [format, setFormat] = useState<PasteFormat>(
    fork.current?.format && isPasteFormat(fork.current.format)
      ? fork.current.format
      : "plain_text",
  );
  const [formatTouched, setFormatTouched] = useState(Boolean(fork.current?.format));
  const [retentionMinutes, setRetentionMinutes] = useState(DEFAULT_API_TTL_MINUTES);
  const [burnAfterReading, setBurnAfterReading] = useState(false);
  const [encryptionOn, setEncryptionOn] = useState(false);
  const [encryptionKey, setEncryptionKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [encryptionOpen, setEncryptionOpen] = useState(false);
  const [createdId, setCreatedId] = useState<string | null>(null);
  const [shareKey, setShareKey] = useState("");
  const [createdBurn, setCreatedBurn] = useState(false);
  const [pending, setPending] = useState(false);
  const [copied, setCopied] = useState(false);
  const [dragging, setDragging] = useState(false);

  const shareLink = useMemo(() => {
    if (!createdId) return null;
    return buildShareUrl(`/p/${createdId}`, shareKey || undefined);
  }, [createdId, shareKey]);

  const bytes = byteLength(content);
  const nearLimit = bytes > MAX_PASTE_BYTES * 0.9;
  const overLimit = bytes > MAX_PASTE_BYTES;

  const applyContent = useCallback(
    (next: string, detected?: PasteFormat | null) => {
      setContent(next);
      if (!formatTouched) {
        const guessed = detected ?? detectFormat(next);
        if (guessed) setFormat(guessed);
      }
    },
    [formatTouched],
  );

  const loadFiles = useCallback(
    async (files: FileList) => {
      const file = files[0];
      if (!file) return;
      try {
        const text = await readTextFile(file, MAX_PASTE_BYTES);
        const fromName = formatFromFilename(file.name);
        if (fromName) {
          setFormat(fromName);
          setFormatTouched(true);
        }
        applyContent(text, fromName);
        toast.message(`Loaded ${file.name}`);
      } catch (error) {
        toast.error(error instanceof Error ? error.message : "Unable to read that file");
      }
    },
    [applyContent],
  );

  const submit = useCallback(async () => {
    if (!content.trim()) {
      toast.error("Content is required");
      return;
    }
    if (overLimit) {
      toast.error("Paste exceeds the 1 MiB limit");
      return;
    }
    if (encryptionOn) {
      const error = validateEncryptionKey(encryptionKey);
      if (error) {
        toast.error("Invalid encryption key", { description: error });
        setEncryptionOpen(true);
        return;
      }
    }

    setPending(true);
    setCreatedId(null);
    setCopied(false);
    try {
      let payloadContent = content;
      let salt: string | null = null;
      let nonce: string | null = null;
      let algorithm: string | null = null;
      const usedKey = encryptionOn ? encryptionKey : "";
      if (encryptionOn) {
        const encrypted = await encryptPaste(content, encryptionKey);
        payloadContent = encrypted.content;
        salt = encrypted.salt;
        nonce = encrypted.nonce;
        algorithm = encrypted.algorithm;
      }
      const result = await createPaste({
        data: {
          content: payloadContent,
          format,
          encrypted: encryptionOn,
          algorithm,
          salt,
          nonce,
          burnAfterReading,
          retentionMinutes,
        },
      });
      const expiresAt =
        retentionMinutes > 0
          ? new Date(Date.now() + retentionMinutes * 60_000).toISOString()
          : null;
      rememberPaste({
        id: result.id,
        format,
        encrypted: encryptionOn,
        burnAfterReading,
        createdAt: new Date().toISOString(),
        expiresAt,
        preview: encryptionOn ? "Encrypted paste" : previewFrom(content),
      });
      const url = buildShareUrl(`/p/${result.id}`, usedKey || undefined);
      const copiedOk = await copyText(url);
      setShareKey(usedKey);
      setCreatedId(result.id);
      setCreatedBurn(burnAfterReading);
      setCopied(copiedOk);
      setContent("");
      setFormat("plain_text");
      setFormatTouched(false);
      setEncryptionKey("");
      setShowKey(false);
      if (encryptionOn) setEncryptionOn(false);
      toast.success("Paste created", {
        description: copiedOk ? "Share URL copied" : undefined,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Failed to create paste", { description: message });
    } finally {
      setPending(false);
    }
  }, [burnAfterReading, content, encryptionKey, encryptionOn, format, overLimit, retentionMinutes]);

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    void submit();
  };

  useHotkeys({ shortcut: "mod+enter", handler: () => formRef.current?.requestSubmit() });

  const generateKey = () => {
    try {
      const key = generateEncryptionKey();
      setEncryptionKey(key);
      setEncryptionOn(true);
      setEncryptionOpen(true);
      setShowKey(true);
      toast.message("256-bit key generated");
    } catch {
      toast.error("Unable to generate a secure key");
    }
  };

  const copyShare = async () => {
    if (!shareLink) return;
    const ok = await copyText(shareLink);
    if (ok) {
      setCopied(true);
      toast.success("Link copied");
    } else {
      toast.error("Unable to copy");
    }
  };

  useEffect(() => {
    if (!encryptionOpen) return;
    const onPointer = () => setEncryptionOpen(false);
    const timer = window.setTimeout(() => {
      window.addEventListener("click", onPointer);
    }, 0);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("click", onPointer);
    };
  }, [encryptionOpen]);

  useEffect(() => {
    if (!content.trim()) return;
    const onLeave = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", onLeave);
    return () => window.removeEventListener("beforeunload", onLeave);
  }, [content]);

  const byteClass = cn(
    "font-mono text-xs tabular-nums",
    overLimit ? "text-danger" : nearLimit ? "text-danger/80" : "text-muted-foreground",
  );

  return (
    <form ref={formRef} className="flex min-h-0 flex-1 flex-col" onSubmit={onSubmit}>
      {shareLink && (
        <section
          className="shrink-0 space-y-3 border-b border-border bg-gutter px-4 py-3 pr-16 sm:px-5 sm:pr-5"
          aria-label="Paste created"
        >
          <div className="flex items-center gap-2">
            <Check className="size-4 text-success" aria-hidden="true" />
            <h2 className="font-mono text-xs font-medium tracking-tight">Paste created</h2>
          </div>
          <div className="flex gap-2">
            <input
              id="share-url"
              readOnly
              value={shareLink}
              onFocus={(event) => event.target.select()}
              className={cn(inputClass, "min-h-11 font-mono text-xs sm:min-h-9")}
            />
            <button
              type="button"
              onClick={() => void copyShare()}
              className="inline-flex size-11 shrink-0 items-center justify-center border border-border text-muted-foreground transition-colors duration-150 hover:text-foreground sm:size-9"
              aria-label="Copy link"
              title="Copy link"
            >
              {copied ? <Check className="size-4 text-success" /> : <Copy className="size-4" />}
            </button>
            <button
              type="button"
              onClick={() => {
                if (!createdId) {
                  navigate({ to: "/" });
                  return;
                }
                void navigate({
                  to: "/p/$id",
                  params: { id: createdId },
                  hash: shareKey ? encodeURIComponent(shareKey) : undefined,
                });
              }}
              className="inline-flex size-11 shrink-0 items-center justify-center border border-border text-muted-foreground transition-colors duration-150 hover:text-foreground sm:size-9"
              aria-label="Open paste"
              title="Open paste"
            >
              <Share2 className="size-4" />
            </button>
          </div>
          {shareKey && (
            <p className="text-xs text-muted-foreground">
              The decryption key is in the fragment after #. Share the full URL only with people who
              should read this paste.
            </p>
          )}
          {createdBurn && (
            <p className="text-xs text-danger">
              Burn is armed. The first successful view consumes this paste.
            </p>
          )}
          {createdId && (
            <div className="hidden gap-2 sm:grid sm:grid-cols-2">
              <CopyField
                id="agent-cli"
                label="cli"
                value={`copypaste get ${createdId} --host ${typeof window === "undefined" ? "" : window.location.origin}${shareKey ? ` --key ${shareKey}` : ""} --json`}
              />
              <CopyField
                id="agent-curl"
                label="curl"
                value={`curl -sS ${typeof window === "undefined" ? "" : window.location.origin}/api/v1/pastes/${createdId}`}
              />
            </div>
          )}
        </section>
      )}

      <CodeEditor
        value={content}
        onChange={(next) => applyContent(next)}
        placeholder="Write or paste"
        hint="Client-side AES-256 · optional burn · 24h default"
        onFiles={(files) => void loadFiles(files)}
        dragging={dragging}
        onDraggingChange={setDragging}
        className="min-h-0 flex-1"
      />

      <div className="shrink-0 border-t border-border bg-gutter pb-[max(0.5rem,env(safe-area-inset-bottom))] sm:pb-[max(0px,env(safe-area-inset-bottom))]">
        <div className="flex items-center gap-0.5 px-2 pr-16 sm:h-10 sm:gap-1 sm:px-3 sm:pr-3">
          <MetaSelect
            id="format"
            label="Format"
            value={format}
            onChange={(value) => {
              setFormatTouched(true);
              setFormat(value as PasteFormat);
            }}
          >
            {PASTE_FORMATS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </MetaSelect>

          <MetaSelect
            id="retention"
            label="Retention"
            value={String(retentionMinutes)}
            onChange={(value) => setRetentionMinutes(Number(value))}
          >
            {RETENTION_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </MetaSelect>

          <button
            type="button"
            role="switch"
            aria-checked={burnAfterReading}
            onClick={() => setBurnAfterReading((value) => !value)}
            title="Burn after reading"
            className={cn(
              ghostBtn,
              burnAfterReading ? "text-danger" : "text-muted-foreground hover:text-foreground",
            )}
          >
            <Flame className="size-4 sm:size-3.5" />
            <span className="hidden sm:inline">burn</span>
          </button>

          <div className="relative shrink-0">
            <button
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                setEncryptionOpen((value) => !value);
              }}
              aria-expanded={encryptionOpen}
              aria-pressed={encryptionOn}
              aria-label="Encryption options"
              title="Encrypt in this browser"
              className={cn(
                ghostBtn,
                encryptionOn ? "text-foreground" : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Lock className="size-4 sm:size-3.5" />
              <span className="hidden sm:inline">{encryptionOn ? "aes" : "lock"}</span>
            </button>
            {encryptionOpen && (
              <>
                <button
                  type="button"
                  aria-label="Dismiss encryption"
                  className="fixed inset-0 z-40 bg-background/70 sm:hidden"
                  onClick={() => setEncryptionOpen(false)}
                />
                <div
                  role="dialog"
                  aria-label="Encryption settings"
                  onClick={(event) => event.stopPropagation()}
                  className="fixed inset-x-0 bottom-0 z-50 space-y-4 border-t border-border bg-background p-5 pr-16 pb-[max(1.25rem,env(safe-area-inset-bottom))] sm:absolute sm:inset-auto sm:bottom-full sm:right-0 sm:z-20 sm:mb-1 sm:w-[min(20rem,calc(100vw-5rem))] sm:border sm:p-4 sm:pr-4 sm:pb-4"
                >
                  <p className="text-xs text-muted-foreground">
                    Encryption runs in this browser. Ciphertext is stored; the key stays in the
                    share URL fragment.
                  </p>
                  <label className="flex min-h-11 items-center gap-3 text-sm sm:min-h-0 sm:gap-2">
                    <input
                      type="checkbox"
                      checked={encryptionOn}
                      onChange={(event) => {
                        const on = event.target.checked;
                        setEncryptionOn(on);
                        if (on && !encryptionKey) {
                          setEncryptionKey(generateEncryptionKey());
                          setShowKey(true);
                        }
                      }}
                      className="size-5 accent-accent sm:size-4"
                    />
                    AES-256-GCM
                  </label>
                  <div className="space-y-1.5">
                    <label className={fieldLabel} htmlFor="encryptionKey">
                      encryption key
                    </label>
                    <div className="flex flex-col gap-2 sm:flex-row">
                      <div className="relative min-w-0 flex-1">
                        <input
                          id="encryptionKey"
                          type={showKey ? "text" : "password"}
                          autoComplete="new-password"
                          value={encryptionKey}
                          disabled={!encryptionOn}
                          onChange={(event) => setEncryptionKey(event.target.value)}
                          placeholder={encryptionOn ? "Shared secret" : "Enable encryption first"}
                          className={cn(inputClass, "min-h-12 pr-10 font-mono text-base sm:min-h-0 sm:text-sm")}
                        />
                        <button
                          type="button"
                          disabled={!encryptionOn}
                          onClick={() => setShowKey((value) => !value)}
                          className="absolute right-1 top-1/2 inline-flex size-10 -translate-y-1/2 items-center justify-center text-muted-foreground hover:text-foreground disabled:opacity-40"
                          aria-label={showKey ? "Hide encryption key" : "Show encryption key"}
                        >
                          {showKey ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
                        </button>
                      </div>
                      <button
                        type="button"
                        onClick={generateKey}
                        className="inline-flex h-12 w-full shrink-0 items-center justify-center border border-border text-sm font-medium transition-colors duration-150 hover:bg-muted sm:h-9 sm:w-auto sm:px-3 sm:text-xs"
                      >
                        Generate
                      </button>
                    </div>
                  </div>
                </div>
              </>
            )}
          </div>

          <div className="ml-auto hidden items-center gap-3 sm:flex">
            <span className={byteClass}>{formatBytes(bytes)}</span>
            <button
              type="submit"
              disabled={pending || overLimit}
              title="Create paste (⌘⏎)"
              className={cn(primaryBtn, "h-8 px-3 text-xs")}
            >
              {pending ? "Creating…" : "Create"}
              <kbd className="font-mono text-2xs opacity-70" aria-hidden="true">
                ⌘⏎
              </kbd>
            </button>
          </div>
        </div>

        <div className="flex items-center gap-3 px-3 pr-16 pt-1 sm:hidden">
          <span className={byteClass}>{formatBytes(bytes)}</span>
          <button
            type="submit"
            disabled={pending || overLimit}
            className={cn(primaryBtn, "h-12 flex-1 text-base")}
          >
            {pending ? "Creating…" : "Create paste"}
          </button>
        </div>
      </div>
    </form>
  );
}

function MetaSelect({
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
    <div className="relative min-w-0 flex-1 sm:flex-none">
      <label className="sr-only" htmlFor={id}>
        {label}
      </label>
      <select
        id={id}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className={selectChip}
      >
        {children}
      </select>
      <ChevronDown className="pointer-events-none absolute right-1 top-1/2 size-3 -translate-y-1/2 text-muted-foreground" />
    </div>
  );
}

function CopyField({ id, label, value }: { id: string; label: string; value: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="space-y-1.5">
      <label className={fieldLabel} htmlFor={id}>
        {label}
      </label>
      <div className="flex gap-2">
        <input
          id={id}
          readOnly
          value={value}
          onFocus={(event) => event.target.select()}
          className={cn(inputClass, "font-mono text-xs")}
        />
        <button
          type="button"
          onClick={async () => {
            if (await copyText(value)) setCopied(true);
          }}
          className="inline-flex size-10 shrink-0 items-center justify-center border border-border text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground"
          aria-label={`Copy ${label}`}
        >
          {copied ? <Check className="size-4 text-success" /> : <Copy className="size-4" />}
        </button>
      </div>
    </div>
  );
}

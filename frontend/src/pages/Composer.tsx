import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Editor } from "@monaco-editor/react";
import { toast } from "sonner";
import { Header } from "../components/Header";
import { CopyButton } from "../components/CopyButton";
import { EncryptToggle } from "../components/EncryptToggle";
import { BurnToggle } from "../components/BurnToggle";
import { LanguageSelect } from "../components/LanguageSelect";
import { ExpireSelect } from "../components/ExpireSelect";
import { createPaste } from "../api/client";
import type { PasteFormat } from "../api/types";

export function Composer() {
  const [content, setContent] = useState("");
  const [format, setFormat] = useState<PasteFormat>("plain_text");
  const [expire, setExpire] = useState("1440");
  const [encrypt, setEncrypt] = useState(false);
  const [burn, setBurn] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async () => {
    if (!content.trim()) {
      toast.error("Nothing to paste");
      return;
    }
    setLoading(true);
    try {
      let encryptionKey: string | null = null;
      let encryptionPayload:
        | { algorithm: "aes256_gcm"; key: string }
        | undefined;
      if (encrypt) {
        encryptionKey = Array.from(crypto.getRandomValues(new Uint8Array(24)))
          .map((b) => b.toString(36).padStart(2, "0"))
          .join("")
          .slice(0, 32);
        encryptionPayload = { algorithm: "aes256_gcm", key: encryptionKey };
      }

      const retentionMinutes = Number(expire);
      const resp = await createPaste({
        content,
        format,
        ...(retentionMinutes > 0
          ? { retention_minutes: retentionMinutes }
          : {}),
        ...(burn ? { burn_after_reading: true } : {}),
        ...(encryptionPayload ? { encryption: encryptionPayload } : {}),
      });

      let pasteUrl = `${window.location.origin}${resp.path}`;
      if (encryptionKey)
        pasteUrl += `#key=${encodeURIComponent(encryptionKey)}`;
      setUrl(pasteUrl);
    } catch {
      toast.error("Failed to create paste");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-[#0a0a0a] text-[#fafafa] flex flex-col">
      <Header />

      <main className="flex-1 flex flex-col p-4 gap-3 max-w-5xl mx-auto w-full">
        <AnimatePresence mode="wait">
          {!url ? (
            <motion.div
              key="editor"
              className="flex-1 flex flex-col gap-3"
              exit={{ opacity: 0, y: -20 }}
              transition={{ duration: 0.15 }}
            >
              <div className="flex-1 rounded-lg border border-[#262626] overflow-hidden min-h-[60vh]">
                <Editor
                  defaultLanguage="plaintext"
                  language={format === "plain_text" ? "plaintext" : format}
                  theme="vs-dark"
                  value={content}
                  onChange={(v) => setContent(v ?? "")}
                  options={{
                    minimap: { enabled: false },
                    fontSize: 14,
                    lineNumbers: "off",
                    scrollBeyondLastLine: false,
                    wordWrap: "on",
                    padding: { top: 16, bottom: 16 },
                  }}
                  onMount={(editor) => {
                    editor.addCommand(
                      // Monaco.KeyMod.CtrlCmd | Monaco.KeyCode.Enter
                      2048 | 3,
                      handleSubmit,
                    );
                  }}
                />
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <LanguageSelect value={format} onChange={setFormat} />
                <ExpireSelect value={expire} onChange={setExpire} />
                <EncryptToggle enabled={encrypt} onChange={setEncrypt} />
                <BurnToggle enabled={burn} onChange={setBurn} />
                <div className="flex-1" />
                <button
                  onClick={handleSubmit}
                  disabled={loading}
                  className="px-5 py-2 bg-[#3b82f6] hover:bg-[#2563eb] disabled:opacity-50
                             rounded-md text-sm font-medium transition-colors"
                >
                  {loading ? "Sharing..." : "Share →"}
                </button>
              </div>
            </motion.div>
          ) : (
            <motion.div
              key="result"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.2 }}
              className="flex-1 flex flex-col items-center justify-center gap-6"
            >
              <div className="text-center">
                <p className="text-[#a1a1a1] text-sm mb-2">
                  Your paste is ready
                </p>
                <div
                  className="flex items-center gap-2 bg-[#141414] border border-[#262626]
                                rounded-lg px-4 py-3 max-w-lg"
                >
                  <code className="text-sm text-[#fafafa] truncate flex-1">
                    {url}
                  </code>
                  <CopyButton text={url} />
                </div>
              </div>
              <div className="text-xs text-[#a1a1a1] font-mono bg-[#141414] px-3 py-2 rounded">
                copypaste get {url.split("/").pop()?.split("#")[0]}
              </div>
              <button
                onClick={() => {
                  setUrl(null);
                  setContent("");
                }}
                className="text-sm text-[#a1a1a1] hover:text-[#fafafa] transition-colors"
              >
                ← New paste
              </button>
            </motion.div>
          )}
        </AnimatePresence>
      </main>
    </div>
  );
}

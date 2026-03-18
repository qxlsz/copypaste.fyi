import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { motion } from "framer-motion";
import { Header } from "../components/Header";
import { CopyButton } from "../components/CopyButton";
import { DecryptForm } from "../components/DecryptForm";
import { fetchPaste } from "../api/client";
import type { PasteViewResponse } from "../server/types";

export function Viewer() {
  const { id } = useParams<{ id: string }>();
  const [paste, setPaste] = useState<PasteViewResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [needsKey, setNeedsKey] = useState(false);

  useEffect(() => {
    if (!id) return;
    const fragment = window.location.hash.replace("#", "");
    const params = new URLSearchParams(fragment);
    const key = params.get("key") ?? undefined;

    fetchPaste(id, key)
      .then(setPaste)
      .catch((e: unknown) => {
        const msg = e instanceof Error ? e.message : "";
        if (msg.includes("401") || msg.includes("403")) {
          setNeedsKey(true);
        } else {
          setError("Paste not found or expired.");
        }
      });
  }, [id]);

  const handleDecrypt = (key: string) => {
    if (!id) return;
    fetchPaste(id, key)
      .then(setPaste)
      .catch(() => setError("Wrong key."));
  };

  const isEncrypted =
    paste?.encryption?.algorithm !== "none" &&
    paste?.encryption?.algorithm !== undefined;

  return (
    <div className="min-h-screen bg-[#0a0a0a] text-[#fafafa] flex flex-col">
      <Header
        actions={
          paste ? (
            <div className="flex gap-2">
              <CopyButton text={paste.content} label="Copy" />
              <a
                href={`/api/pastes/${id}/raw`}
                className="text-sm text-[#a1a1a1] hover:text-[#fafafa]
                        px-3 py-1.5 rounded border border-[#262626] transition-colors"
              >
                Raw
              </a>
              <a
                href="/"
                className="text-sm text-[#a1a1a1] hover:text-[#fafafa]
                        px-3 py-1.5 rounded border border-[#262626] transition-colors"
              >
                + New
              </a>
            </div>
          ) : undefined
        }
      />

      <main className="flex-1 flex flex-col p-4 max-w-5xl mx-auto w-full gap-3">
        {error && (
          <div className="flex-1 flex items-center justify-center">
            <p className="text-[#a1a1a1]">{error}</p>
          </div>
        )}

        {needsKey && !paste && (
          <div className="flex-1 flex items-center justify-center">
            <DecryptForm onSubmit={handleDecrypt} />
          </div>
        )}

        {paste?.burnAfterReading && (
          <div
            className="bg-[#ef4444]/10 border border-[#ef4444]/30 rounded-lg px-4 py-2
                          text-[#ef4444] text-sm"
          >
            🔥 This paste will be destroyed after you close this page.
          </div>
        )}

        {paste && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className="flex-1 flex flex-col gap-3"
          >
            <div className="flex items-center gap-3 text-xs text-[#a1a1a1]">
              <span className="bg-[#262626] px-2 py-0.5 rounded">
                {paste.format ?? "text"}
              </span>
              {paste.expiresAt && (
                <span>expires {formatRelative(paste.expiresAt)}</span>
              )}
              {isEncrypted && (
                <span className="text-[#a855f7]">🔒 encrypted</span>
              )}
            </div>

            <div
              className="flex-1 bg-[#141414] border border-[#262626] rounded-lg
                            overflow-auto p-4 font-mono text-sm whitespace-pre-wrap min-h-[60vh]"
            >
              {paste.content}
            </div>

            <div className="flex items-center gap-2 text-xs text-[#a1a1a1]">
              <code className="font-mono">copypaste get {id}</code>
              <CopyButton text={`copypaste get ${id}`} size="sm" />
            </div>
          </motion.div>
        )}
      </main>
    </div>
  );
}

function formatRelative(unixSeconds: number): string {
  const diff = unixSeconds * 1000 - Date.now();
  const hours = Math.floor(diff / 3600000);
  if (hours > 24) return `in ${Math.floor(hours / 24)}d`;
  if (hours > 0) return `in ${hours}h`;
  return "soon";
}

import { useEffect, useState } from "react";
import { toast } from "sonner";

import {
  ChatGptMark,
  ClaudeMark,
  CodexMark,
  GoogleMark,
  GrokMark,
  WhatsAppMark,
} from "./AgentMarks";
import { API_BASE } from "../api/client";
import { gmailShareHref, whatsappShareHref } from "../lib/googleShare";
import {
  GROK_BOT_ADD_PROMPT,
  GROK_BOT_SKILL,
  OPEN_AGENTS,
  grokBotHref,
  openPrompt,
} from "../lib/openAgents";

const logoButton =
  "inline-flex min-h-11 min-w-11 flex-col items-center justify-center gap-0.5 rounded-md px-1.5 py-1 text-text transition hover:bg-border focus-visible:outline-none";

declare global {
  interface Window {
    google?: {
      accounts: {
        id: {
          initialize: (opts: {
            client_id: string;
            callback: (response: { credential: string }) => void;
          }) => void;
          prompt: () => void;
        };
      };
    };
  }
}

export const OpenWithAgents = ({ url }: { url: string }) => {
  const prompt = openPrompt(url);
  const [googleClientId, setGoogleClientId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetch(`${API_BASE}/auth/providers`, { credentials: "omit" })
      .then((response) => (response.ok ? response.json() : null))
      .then((body: { google?: string | null } | null) => {
        if (!cancelled && typeof body?.google === "string" && body.google) {
          setGoogleClientId(body.google);
        }
      })
      .catch(() => {
        /* public site may not expose providers */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleAddToGrok = async () => {
    try {
      await navigator.clipboard.writeText(GROK_BOT_SKILL);
      toast.success("Grok Bot skill copied");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Unable to copy Grok Bot skill", { description: message });
      return;
    }
    window.open(grokBotHref(GROK_BOT_ADD_PROMPT), "_blank", "noopener,noreferrer");
  };

  const handleGoogleSignIn = async () => {
    if (!googleClientId) {
      window.open(gmailShareHref(url), "_blank", "noopener,noreferrer");
      return;
    }
    const ensureScript = () =>
      new Promise<void>((resolve, reject) => {
        if (window.google?.accounts?.id) {
          resolve();
          return;
        }
        const existing = document.querySelector("script[data-google-gsi]");
        if (existing) {
          existing.addEventListener("load", () => resolve());
          existing.addEventListener("error", () => reject(new Error("Google script")));
          return;
        }
        const script = document.createElement("script");
        script.src = "https://accounts.google.com/gsi/client";
        script.async = true;
        script.dataset.googleGsi = "1";
        script.onload = () => resolve();
        script.onerror = () => reject(new Error("Google script"));
        document.head.appendChild(script);
      });
    try {
      await ensureScript();
      window.google?.accounts.id.initialize({
        client_id: googleClientId,
        callback: (response) => {
          void fetch(`${API_BASE}/auth/google`, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ credential: response.credential }),
          }).then(async (res) => {
            if (!res.ok) {
              toast.error("Google sign-in failed");
              return;
            }
            toast.success("Signed in with Google");
            window.open(gmailShareHref(url), "_blank", "noopener,noreferrer");
          });
        },
      });
      window.google?.accounts.id.prompt();
    } catch {
      window.open(gmailShareHref(url), "_blank", "noopener,noreferrer");
    }
  };

  return (
    <div className="flex flex-wrap items-end gap-1">
      {OPEN_AGENTS.map((agent) => {
        const Mark =
          agent.id === "grok"
            ? GrokMark
            : agent.id === "codex"
              ? CodexMark
              : agent.id === "chatgpt"
                ? ChatGptMark
                : ClaudeMark;
        return (
          <a
            key={agent.id}
            href={agent.href(prompt)}
            target="_blank"
            rel="noopener noreferrer"
            className={logoButton}
            aria-label={agent.label}
            title={`Open in ${agent.label}`}
          >
            <Mark />
            <span className="text-[10px] leading-none text-muted-foreground">{agent.label}</span>
          </a>
        );
      })}
      <a
        href={whatsappShareHref(url)}
        target="_blank"
        rel="noopener noreferrer"
        className={logoButton}
        aria-label="Share with WhatsApp"
        title="Share with WhatsApp"
      >
        <WhatsAppMark />
        <span className="text-[10px] leading-none text-muted-foreground">WhatsApp</span>
      </a>
      <a
        href={gmailShareHref(url)}
        target="_blank"
        rel="noopener noreferrer"
        className={logoButton}
        aria-label="Share with Gmail"
        title="Share with Gmail"
      >
        <GoogleMark />
        <span className="text-[10px] leading-none text-muted-foreground">Gmail</span>
      </a>
      <button
        type="button"
        onClick={() => void handleGoogleSignIn()}
        className={logoButton}
        aria-label="Sign in with Google"
        title="Sign in with Google, then share"
      >
        <GoogleMark />
        <span className="text-[10px] leading-none text-muted-foreground">Google</span>
      </button>
      <button
        type="button"
        onClick={() => void handleAddToGrok()}
        className={logoButton}
        aria-label="Add to Grok"
        title="Add to Grok"
      >
        <span className="relative inline-flex">
          <GrokMark />
          <span className="absolute -right-1.5 -top-1 text-[10px] leading-none">+</span>
        </span>
        <span className="text-[10px] leading-none text-muted-foreground">Add</span>
      </button>
    </div>
  );
};

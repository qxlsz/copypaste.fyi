import { useEffect, useState } from "react";
import { API_BASE } from "../api/client";
import { SelfHostHelper } from "../components/SelfHostHelper";

const features = [
  ["encryption", "AES-256-GCM · ChaCha20 · XChaCha20 · experimental ML-KEM hybrid"],
  ["burn-after-reading", "best-effort deletion after a successful read"],
  ["retention", "1 minute to 30 days, enforced server-side"],
  ["time-locks", "not-before / not-after access windows"],
  ["hardened profile", "bundles, attestations, webhooks, and steganography disabled"],
  ["tor-only", "restrict a paste to .onion access"],
  ["anchoring", "admin-only encrypted-content commitment or plaintext metadata manifest"],
  ["cli", "pipe from your terminal: copypaste send"],
  ["api", "REST + OpenAPI JSON at /api/openapi.json"],
] as const;

interface HealthResponse {
  status: string;
  timestamp: number;
  version: string;
  commit?: string;
}

export const AboutPage = () => {
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  useEffect(() => {
    const fetchHealth = async () => {
      try {
        const response = await fetch(`${API_BASE}/health`, {
          cache: "no-store",
          credentials: "omit",
          redirect: "error",
        });
        if (response.ok) {
          setHealth((await response.json()) as HealthResponse);
        } else {
          setHealthError("health check failed");
        }
      } catch {
        setHealthError("health check unreachable");
      }
    };
    fetchHealth();
  }, []);

  return (
    <article className="mx-auto max-w-2xl space-y-10 pb-16">
      <header className="space-y-3">
        <p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
          copypaste
        </p>
        <h1 className="text-3xl font-medium tracking-tight text-text">A paste that disappears</h1>
        <p className="text-base leading-relaxed text-muted-foreground">
          Open-source paste sharing for secrets that should not outlive their purpose. A Rust
          backend encrypts and enforces retention. There is no public listing.
        </p>
      </header>

      <dl className="divide-y divide-border">
        {features.map(([term, detail]) => (
          <div
            key={term}
            className="flex flex-col gap-1 py-3 sm:flex-row sm:items-baseline sm:gap-8"
          >
            <dt className="w-40 shrink-0 text-sm font-medium text-text">{term}</dt>
            <dd className="text-sm leading-relaxed text-muted-foreground">{detail}</dd>
          </div>
        ))}
      </dl>

      <section className="space-y-3">
        <h2 className="text-base font-medium tracking-tight text-text">Run your own</h2>
        <p className="text-sm leading-relaxed text-muted-foreground">
          Same binary as this site. The helper below picks one recipe. Cookbook:{" "}
          <a
            href="https://github.com/qxlsz/copypaste.fyi/blob/main/docs/self-host.md"
            target="_blank"
            rel="noopener noreferrer"
            className="underline decoration-border underline-offset-4 hover:text-text"
          >
            docs/self-host.md
          </a>
          .
        </p>
        <SelfHostHelper />
      </section>

      <section className="space-y-3">
        <h2 className="text-base font-medium tracking-tight text-text">How it works</h2>
        <pre className="overflow-x-auto font-mono text-xs leading-6 text-muted-foreground">
          {`browser / cli
   → POST /api/pastes            content + policy
   → encrypt                     Rust (aes-gcm · chacha20 · ml-kem)
   → verify                      optional OCaml check
   → store                       in-memory by default · Redis optional
   → share                       /p/<id> — key travels in the #fragment`}
        </pre>
        <p className="text-sm leading-relaxed text-muted-foreground">
          Plain honesty: when you supply an encryption key, the server performs the encryption — the
          key transits over TLS and is never stored. Share keys out of band for anything sensitive.
          Full details in{" "}
          <a
            href="https://github.com/qxlsz/copypaste.fyi/blob/main/docs/encryption.md"
            target="_blank"
            rel="noopener noreferrer"
            className="underline decoration-border underline-offset-4 hover:text-text"
          >
            docs/encryption.md
          </a>
          .
        </p>
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-medium tracking-tight text-text">Status</h2>
        {health ? (
          <p className="font-mono text-xs text-muted-foreground">
            <span className={health.status === "ok" ? "text-success" : "text-danger"}>
              {health.status}
            </span>
            {" · "}v{health.version}
            {health.commit ? ` · ${health.commit.slice(0, 7)}` : ""}
          </p>
        ) : (
          <p className="font-mono text-xs text-muted-foreground">{healthError ?? "checking…"}</p>
        )}
      </section>

      <footer className="flex flex-wrap gap-x-4 gap-y-2 text-xs text-muted-foreground">
        <a
          href="https://github.com/qxlsz/copypaste.fyi"
          target="_blank"
          rel="noopener noreferrer"
          className="hover:text-text"
        >
          github
        </a>
        <a
          href={`${API_BASE}/openapi.json`}
          target="_blank"
          rel="noopener noreferrer"
          className="hover:text-text"
        >
          openapi json
        </a>
        <a
          href="https://github.com/qxlsz/copypaste.fyi/blob/main/SECURITY.md"
          target="_blank"
          rel="noopener noreferrer"
          className="hover:text-text"
        >
          security policy
        </a>
      </footer>
    </article>
  );
};

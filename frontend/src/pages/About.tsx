import { useEffect, useState } from "react";
import { API_BASE } from "../api/client";

const features = [
  ["encryption", "AES-256-GCM · ChaCha20 · XChaCha20 · ML-KEM-768 hybrid"],
  ["burn-after-reading", "deleted on first successful read"],
  ["retention", "1 minute to 30 days, enforced server-side"],
  ["time-locks", "not-before / not-after access windows"],
  ["attestation", "TOTP or shared-secret gate before viewing"],
  ["steganography", "hide ciphertext inside PNG carrier images"],
  ["bundles", "group related pastes under one link"],
  ["webhooks", "Slack / Teams / generic notifications on view & burn"],
  ["tor-only", "restrict a paste to .onion access"],
  ["anchoring", "opt-in blockchain manifest anchoring"],
  ["cli", "pipe from your terminal: copypaste send"],
  ["api", "REST + OpenAPI, docs at /api/docs"],
] as const;

interface HealthResponse {
  status: string;
  timestamp: number;
  version: string;
  commit?: string;
  services: {
    backend: { status: string };
    crypto_verifier: { status: string };
    storage: { status: string };
  };
}

const SectionLabel = ({ children }: { children: React.ReactNode }) => (
  <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
    {children}
  </p>
);

const HealthChip = ({ label, status }: { label: string; status: string }) => (
  <span className="inline-flex items-center gap-1.5 rounded border border-border px-2 py-1 font-mono text-[11px] text-muted-foreground">
    <span
      aria-hidden="true"
      className={`inline-block h-1.5 w-1.5 rounded-full ${
        status === "ok" ? "bg-success" : "bg-danger"
      }`}
    />
    {label}: {status}
  </span>
);

export const AboutPage = () => {
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  useEffect(() => {
    const fetchHealth = async () => {
      try {
        const response = await fetch(`${API_BASE}/health`);
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
    <div className="mx-auto max-w-2xl space-y-12 pb-16">
      <header className="space-y-3">
        <h1 className="font-mono text-xl font-semibold tracking-tight text-text">
          about
        </h1>
        <p className="text-sm leading-relaxed text-muted-foreground">
          copypaste.fyi is an open-source paste sharing service built for
          secrets that should not outlive their purpose. A Rust backend
          encrypts and enforces retention; an independent OCaml service
          re-verifies the cryptography; the frontend stays out of the way.
        </p>
      </header>

      <section className="space-y-4">
        <SectionLabel>Features</SectionLabel>
        <dl className="divide-y divide-border rounded-lg border border-border bg-surface">
          {features.map(([term, detail]) => (
            <div
              key={term}
              className="flex flex-col gap-0.5 px-4 py-2.5 sm:flex-row sm:items-baseline sm:gap-4"
            >
              <dt className="w-40 shrink-0 font-mono text-xs text-text">
                {term}
              </dt>
              <dd className="text-sm text-muted-foreground">{detail}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section className="space-y-4">
        <SectionLabel>How it works</SectionLabel>
        <pre className="overflow-x-auto rounded-lg border border-border bg-surface p-4 font-mono text-xs leading-6 text-muted-foreground">
          {`browser / cli
   → POST /api/pastes            content + policy (retention, burn, locks)
   → encrypt                     Rust (aes-gcm · chacha20 · ml-kem crates)
   → verify                      independent OCaml re-check (mirage-crypto)
   → store                       in-memory by default · Redis / Vault optional
   → share                       /p/<id> — key travels in the #fragment`}
        </pre>
        <p className="text-sm leading-relaxed text-muted-foreground">
          Plain honesty: when you supply an encryption key, the server performs
          the encryption — the key transits over TLS and is never stored. Share
          keys out of band for anything sensitive. Full details in{" "}
          <a
            href="https://github.com/qxlsz/copypaste.fyi/blob/main/docs/encryption.md"
            target="_blank"
            rel="noopener noreferrer"
            className="text-accent underline-offset-2 hover:underline"
          >
            docs/encryption.md
          </a>
          .
        </p>
      </section>

      <section className="space-y-4">
        <SectionLabel>Status</SectionLabel>
        <div className="rounded-lg border border-border bg-surface p-4">
          {health ? (
            <div className="flex flex-wrap items-center gap-2">
              <HealthChip label="overall" status={health.status} />
              <HealthChip label="backend" status={health.services.backend.status} />
              <HealthChip
                label="crypto-verifier"
                status={health.services.crypto_verifier.status}
              />
              <HealthChip label="storage" status={health.services.storage.status} />
              <span className="font-mono text-[11px] text-muted-foreground">
                v{health.version}
                {health.commit ? ` · ${health.commit.slice(0, 7)}` : ""}
              </span>
            </div>
          ) : (
            <p className="font-mono text-xs text-muted-foreground">
              {healthError ?? "checking…"}
            </p>
          )}
        </div>
      </section>

      <footer className="flex flex-wrap gap-4 font-mono text-xs text-muted-foreground">
        <a
          href="https://github.com/qxlsz/copypaste.fyi"
          target="_blank"
          rel="noopener noreferrer"
          className="hover:text-text"
        >
          github
        </a>
        <a href="/api/docs" className="hover:text-text">
          api docs
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
    </div>
  );
};

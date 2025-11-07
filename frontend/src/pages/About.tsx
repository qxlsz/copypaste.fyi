import { MermaidDiagram } from '../components/MermaidDiagram'

const securityPillars = [
  {
    title: 'Non-Extractable Identity',
    description:
      'Authentication is anchored by Ed25519 challenge-response. Private keys live in IndexedDB-backed vaults and never leave the client.',
    accent: 'from-sky-500 to-indigo-600',
  },
  {
    title: 'Zero-Knowledge Persistence',
    description:
      'The backend persists ciphertext, retention policy, and a hashed public key. No plaintext, filenames, or MIME hints ever land on disk.',
    accent: 'from-emerald-500 to-teal-600',
  },
  {
    title: 'Deterministic Auditing',
    description:
      'Every mutation is signed and replay-protected so operators can verify access events without exposing content.',
    accent: 'from-purple-500 to-fuchsia-600',
  },
  {
    title: 'Policy-Enforced Delivery',
    description:
      'Burn-after-reading, temporal windows, Tor-only routing, and webhook attestations compose atomically inside the Rocket request pipeline.',
    accent: 'from-amber-500 to-orange-600',
  },
]

const architectureDiagram = `
flowchart TD
  Client[Client Layer<br/>React SPA + Crypto]
  Edge[Edge Distribution<br/>CDN + Tor]
  Backend[Backend Core<br/>API + Storage]
  Workers[Async Workers<br/>Bundling & Webhooks]

  Client --> Edge
  Edge --> Backend
  Backend --> Workers
  Workers --> Backend
`

const lifecycleDiagram = `
sequenceDiagram
  participant UA as User Agent
  participant CC as Client Crypto
  participant API as Rocket API
  participant DB as Persistence
  participant Hooks as Webhooks

  Note over UA,CC: Local-only key material
  UA->>CC: Generate / load Ed25519 keypair
  UA->>CC: Compose payload + policy JSON
  CC->>CC: Derive XChaCha20 key (HKDF)
  CC->>CC: Encrypt payload & metadata
  CC->>API: POST /api/pastes {ciphertext, policy, signature}
  API->>API: Validate signature & timestamps
  API->>DB: Persist ciphertext + retention controls
  API-->>Hooks: Emit optional webhook notification
  API-->>UA: Return locator, policy digest, view key
  UA->>API: Retrieve via signed request
  API-->>UA: Stream ciphertext + metadata envelope
  Note over UA,CC: Decrypt client-side to plaintext
`

const policyDiagram = `
stateDiagram-v2
  classDef initial fill:#dbeafe,stroke:#3b82f6,stroke-width:3px,color:#1e3a8a
  classDef active fill:#d1fae5,stroke:#10b981,stroke-width:3px,color:#064e3b
  classDef terminal fill:#fecaca,stroke:#ef4444,stroke-width:3px,color:#7f1d1d
  classDef archived fill:#e0e7ff,stroke:#6366f1,stroke-width:3px,color:#312e81

  [*] --> Draft : 📝 Client assembles<br/>policy envelope
  
  Draft --> Sealed : 🔒 Payload encrypted<br/>& cryptographically signed
  
  Sealed --> Active : ✅ API verification<br/>succeeds
  
  note right of Active
    Paste is now live and<br/>
    accessible via signed<br/>
    requests with valid key
  end note
  
  Active --> Burned : 🔥 Burn-after-reading<br/>limit consumed
  Active --> Expired : ⏱️ Retention window<br/>elapsed
  Active --> Archived : 📦 Governance export<br/>triggered
  
  Burned --> [*] : 💨 Permanently deleted
  Expired --> [*] : 🗑️ Removed from storage
  Archived --> [*] : 📁 Sealed in cold storage

  class Draft,Sealed initial
  class Active active
  class Burned,Expired terminal
  class Archived archived
`

const useCases = [
  {
    title: 'Secure infra handoffs',
    detail:
      'Ephemeral kubeconfigs, TLS bundles, or API credentials distributed with automatic revocation and read auditing.',
  },
  {
    title: 'Cryptographic code review',
    detail:
      'Exploit proofs or emergency patches stay encrypted until reviewers decrypt locally with verified signatures.',
  },
  {
    title: 'Incident response playbooks',
    detail:
      'CSIRTs coordinate sensitive indicators of compromise with enforced burn-after-reading behaviour and attestation.',
  },
  {
    title: 'Anonymous disclosures',
    detail:
      'Whistleblowers publish verifiable documents through Tor-only routes while retaining cryptographic proof of origin.',
  },
]

const roadmapItems = [
  {
    title: 'Post-quantum migration',
    detail:
      'Introduce hybrid trust anchors with CRYSTALS-Dilithium signatures and Kyber KEM alongside Ed25519 to ease transition.',
  },
  {
    title: 'Federated deployments',
    detail:
      'Peer discovery and replication protocol so sovereign operators can exchange encrypted pastes with policy attestation.',
  },
  {
    title: 'HSM-backed custodianship',
    detail:
      'Optional PKCS#11 and AWS CloudHSM adapters for environments that require hardware-rooted signing and key custody.',
  },
  {
    title: 'Formal verification',
    detail:
      'Model authentication and policy flows in TLA+ and ProVerif to mechanically prove forward secrecy and non-repudiation.',
  },
]

export const AboutPage = () => {
  return (
    <div className="min-h-screen bg-gradient-to-b from-slate-50 via-slate-100 to-slate-200 dark:from-[#05070f] dark:via-[#0b1120] dark:to-[#0f172a]">
      <div className="mx-auto max-w-6xl px-4 pb-24 pt-16 sm:px-6 lg:px-8">
        <header className="space-y-6 text-slate-800 dark:text-slate-200">
          <span className="inline-flex items-center rounded-full bg-indigo-50 px-3 py-1 text-xs font-semibold uppercase tracking-[0.3em] text-indigo-600 dark:bg-indigo-500/10 dark:text-indigo-200">
            Zero-knowledge paste infrastructure
          </span>
          <h1 className="font-display text-4xl font-semibold tracking-tight text-slate-900 dark:text-slate-50 sm:text-5xl">
            Why copypaste.fyi exists
          </h1>
          <p className="max-w-3xl text-lg leading-relaxed text-slate-600 dark:text-slate-300">
            The platform eliminates plaintext transport and shared credentials by pushing all cryptographic authority to the client. Each paste
            becomes a sealed artifact with verifiable provenance, strict retention rules, and tooling designed for operators who treat secrets as
            production workloads.
          </p>
        </header>

        <main className="mt-14 space-y-24">
          <section className="grid gap-6 sm:grid-cols-2">
            {securityPillars.map(pillar => (
              <article
                key={pillar.title}
                className="relative overflow-hidden rounded-2xl border border-white/70 bg-white/80  p-6 shadow-lg backdrop-blur-xl transition hover:-translate-y-1 hover:shadow-xl dark:border-white/10 dark:bg-white/5"
              >
                <div className={`absolute inset-x-6 top-0 h-1 rounded-b-full bg-gradient-to-r ${pillar.accent}`} aria-hidden="true" />
                <h2 className="text-xl font-semibold text-slate-900 dark:text-slate-100">{pillar.title}</h2>
                <p className="mt-3 text-sm leading-relaxed text-slate-600 dark:text-slate-300/90">{pillar.description}</p>
              </article>
            ))}
          </section>

          <section className="grid items-start gap-12 lg:grid-cols-[1.1fr_0.9fr]">
            <div className="space-y-6">
              <div>
                <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">Architecture overview</h2>
                <p className="mt-3 text-base text-slate-600 dark:text-slate-300/90">
                  A Vite-compiled React SPA runs entirely client-side crypto. Users authenticate with Ed25519 signatures while payloads are sealed with
                  XChaCha20-Poly1305 before any network hop. Rocket enforces policies, persists ciphertext with RocksDB, and streams through Fly.io and
                  Tor without observing plaintext.
                </p>
              </div>
              <MermaidDiagram
                id="architecture-flow"
                chart={architectureDiagram}
                ariaLabel="System architecture"
                title="Client, edge, and core services"
                description="Trace how sealed payloads travel from the browser to edge POPs and the Rocket core before resting in persistence or relaying to asynchronous workers."
              />
            </div>

            <div className="rounded-2xl border border-slate-200/60 bg-white/90 p-8 shadow-lg transition dark:border-white/10 dark:bg-white/5">
              <h3 className="text-lg font-medium text-indigo-900 dark:text-indigo-100">Design commitments</h3>
              <ul className="mt-4 space-y-4 text-sm text-slate-600 dark:text-slate-300/90">
                <li>
                  <strong className="font-semibold text-slate-900 dark:text-slate-100">Client-held secrets.</strong> No password resets and no recovery
                  flow—losing the private key is equivalent to losing identity.
                </li>
                <li>
                  <strong className="font-semibold text-slate-900 dark:text-slate-100">Provable integrity.</strong> All API requests are signed and
                  replay-protected with monotonic nonces and timestamp validation.
                </li>
                <li>
                  <strong className="font-semibold text-slate-900 dark:text-slate-100">Composable policies.</strong> Burn-after-reading, retention windows,
                  webhook attestations, and Tor-only access can be layered without metadata leakage.
                </li>
                <li>
                  <strong className="font-semibold text-slate-900 dark:text-slate-100">Observability without surveillance.</strong> Metrics focus on latency
                  and success rates—no user-level analytics or IP logging.
                </li>
              </ul>
            </div>
          </section>

          <section className="space-y-6">
            <div>
              <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">Lifecycle of a paste</h2>
              <p className="mt-3 max-w-3xl text-base text-slate-600 dark:text-slate-300/90">
                Every submission is cryptographically self-contained: the browser derives ephemeral keys, signs the payload, and ships sealed content to
                the API. Rocket verifies every step before persistence or streaming back to the caller.
              </p>
            </div>
            <MermaidDiagram
              id="paste-sequence"
              chart={lifecycleDiagram}
              ariaLabel="Paste lifecycle sequence diagram"
              title="Submission and retrieval flow"
              description="Challenge–response authentication, envelope sealing, optional webhook notifications, and decrypted retrieval flow."
            />
          </section>

          <section className="space-y-6">
            <div>
              <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">Policy state machine</h2>
              <p className="mt-3 max-w-3xl text-base text-slate-600 dark:text-slate-300/90">
                Enforcement is deterministic: once payloads are sealed, Rocket promotes them through evaluated states and prevents resurrection of
                burned or expired artifacts—even if the persistence layer is tampered with.
              </p>
            </div>
            <MermaidDiagram
              id="policy-machine"
              chart={policyDiagram}
              ariaLabel="Policy enforcement state machine"
              title="Policy transition graph"
              description="Deterministic state progression driven by burn-after-reading, retention windows, or governance archival."
              defaultOpen={false}
            />
          </section>

          <section>
            <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">Operational use cases</h2>
            <div className="mt-6 grid gap-6 md:grid-cols-2">
              {useCases.map(item => (
                <article
                  key={item.title}
                  className="rounded-2xl border border-slate-200/70 bg-white/95 p-6 shadow-md transition hover:-translate-y-0.5 hover:shadow-lg dark:border-slate-700/60 dark:bg-slate-900/70"
                >
                  <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">{item.title}</h3>
                  <p className="mt-3 text-sm leading-relaxed text-slate-600 dark:text-slate-300/90">{item.detail}</p>
                </article>
              ))}
            </div>
          </section>

          <section>
            <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">Roadmap</h2>
            <div className="mt-6 grid gap-6 md:grid-cols-2">
              {roadmapItems.map(item => (
                <article
                  key={item.title}
                  className="rounded-2xl border border-slate-200/70 bg-white/95 p-6 shadow-md transition hover:-translate-y-0.5 hover:shadow-lg dark:border-slate-700/60 dark:bg-slate-900/70"
                >
                  <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">{item.title}</h3>
                  <p className="mt-3 text-sm leading-relaxed text-slate-600 dark:text-slate-300/90">{item.detail}</p>
                </article>
              ))}
            </div>
          </section>

          <section className="rounded-3xl border border-white/80 bg-white/85 p-8 shadow-xl backdrop-blur-xl dark:border-white/10 dark:bg-white/5">
            <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">Transparency & verification</h2>
            <p className="mt-4 text-base text-slate-600 dark:text-slate-300/90">
              Every repository, deployment manifest, and cryptographic primitive is published for review. Threat models, reproducible build scripts, and
              signed releases ship alongside the codebase so researchers can validate implementations instead of trusting claims.
            </p>
            <p className="mt-4 text-base text-slate-600 dark:text-slate-300/90">
              Responsible disclosure channels provide encrypted communication paths. Security advisories are signed with the project key to guarantee
              authenticity, and regression tests codify discovered issues to prevent recurrence.
            </p>
          </section>
        </main>
      </div>
    </div>
  )
}

import { MermaidDiagram } from "../components/MermaidDiagram";

const securityPillars = [
  {
    title: "Non-Extractable Identity",
    description:
      "Authentication is anchored by Ed25519 challenge-response. Private keys live in IndexedDB-backed vaults and never leave the client.",
    accent: "from-sky-500 to-indigo-600",
  },
  {
    title: "Zero-Knowledge Persistence",
    description:
      "The backend persists ciphertext, retention policy, and a hashed public key. No plaintext, filenames, or MIME hints ever land on disk.",
    accent: "from-emerald-500 to-teal-600",
  },
  {
    title: "Dual Cryptographic Verification",
    description:
      "Each encryption operation is independently verified by both the primary Rust implementation and a secondary OCaml service using mirage-crypto, providing defense-in-depth security assurance.",
    accent: "from-rose-500 to-pink-600",
  },
  {
    title: "Deterministic Auditing",
    description:
      "Every mutation is signed and replay-protected so operators can verify access events without exposing content.",
    accent: "from-purple-500 to-fuchsia-600",
  },
  {
    title: "Policy-Enforced Delivery",
    description:
      "Burn-after-reading, temporal windows, Tor-only routing, and webhook attestations compose atomically inside the Rocket request pipeline.",
    accent: "from-amber-500 to-orange-600",
  },
];

const architectureDiagram = `
graph TD
  %% Client Layer
  A1[React SPA<br/>Vite + TypeScript]
  A2[WebCrypto API<br/>Ed25519 Keys]
  A3[IndexedDB Vault<br/>Key Storage]

  %% Edge Layer
  B1[Vercel Edge CDN<br/>Global POPs]
  B2[Tor Hidden Service<br/>.onion Gateway]

  %% Backend Layer
  C1[Rocket API<br/>Auth & Policy]
  C2[Rust Crypto Engine<br/>Primary Verification]
  C3[OCaml Crypto Service<br/>Secondary Verification<br/>mirage-crypto]
  C4[RocksDB Storage<br/>Ciphertext Store]

  %% Worker Layer
  D1[Async Workers<br/>Bundle Assembly]
  D2[Webhook Delivery<br/>Event Notifications]

  %% Client connections
  A1 --> A2
  A2 --> A3
  A3 --> A1

  %% Edge connections
  A1 -->|HTTPS + HSTS| B1
  A1 -->|Direct Signed| C1
  B1 -->|Load Balanced| C1
  C1 -.->|Anonymous Access| B2

  %% Backend connections
  C1 -->|Encrypt & Verify| C2
  C2 -->|Dual Verification| C3
  C2 -->|Store Ciphertext| C4
  C1 -->|Dispatch Jobs| D1
  C1 -->|Send Events| D2

  %% Worker connections
  D1 -->|Read/Write| C4
  D2 -->|HTTP Callbacks| B1

  %% Styling
  classDef client fill:#dbeafe,stroke:#2563eb,stroke-width:2px,color:#1e40af
  classDef edge fill:#f3e8ff,stroke:#7c3aed,stroke-width:2px,color:#6b21a8
  classDef backend fill:#fed7aa,stroke:#ea580c,stroke-width:2px,color:#9a3412
  classDef crypto fill:#fce7f3,stroke:#ec4899,stroke-width:2px,color:#be185d
  classDef worker fill:#ccfbf1,stroke:#0d9488,stroke-width:2px,color:#0f766e

  class A1,A2,A3 client
  class B1,B2 edge
  class C1,C4 backend
  class C2,C3 crypto
  class D1,D2 worker
`;

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
`;

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
`;

const useCases = [
  {
    title: "Secure infra handoffs",
    detail:
      "Ephemeral kubeconfigs, TLS bundles, or API credentials distributed with automatic revocation and read auditing.",
  },
  {
    title: "Cryptographic code review",
    detail:
      "Exploit proofs or emergency patches stay encrypted until reviewers decrypt locally with verified signatures.",
  },
  {
    title: "Incident response playbooks",
    detail:
      "CSIRTs coordinate sensitive indicators of compromise with enforced burn-after-reading behaviour and attestation.",
  },
  {
    title: "Anonymous disclosures",
    detail:
      "Whistleblowers publish verifiable documents through Tor-only routes while retaining cryptographic proof of origin.",
  },
];

const roadmapItems = [
  {
    title: "✅ Post-quantum cryptography (Implemented)",
    detail:
      "Kyber hybrid encryption with AES-256-GCM is now available for quantum-resistant key exchange alongside classical algorithms.",
  },
  {
    title: "Federated deployments",
    detail:
      "Peer discovery and replication protocol so sovereign operators can exchange encrypted pastes with policy attestation.",
  },
  {
    title: "HSM-backed custodianship",
    detail:
      "Optional PKCS#11 and AWS CloudHSM adapters for environments that require hardware-rooted signing and key custody.",
  },
  {
    title: "Formal verification",
    detail:
      "Model authentication and policy flows in TLA+ and ProVerif to mechanically prove forward secrecy and non-repudiation.",
  },
];

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
            The platform eliminates plaintext transport and shared credentials
            by pushing all cryptographic authority to the client. Each paste
            becomes a sealed artifact with verifiable provenance, strict
            retention rules, and tooling designed for operators who treat
            secrets as production workloads.
          </p>
        </header>

        <main className="mt-14 space-y-24">
          <section className="grid gap-6 sm:grid-cols-2">
            {securityPillars.map((pillar) => (
              <article
                key={pillar.title}
                className="relative overflow-hidden rounded-2xl border border-white/70 bg-white/80  p-6 shadow-lg backdrop-blur-xl transition hover:-translate-y-1 hover:shadow-xl dark:border-white/10 dark:bg-white/5"
              >
                <div
                  className={`absolute inset-x-6 top-0 h-1 rounded-b-full bg-gradient-to-r ${pillar.accent}`}
                  aria-hidden="true"
                />
                <h2 className="text-xl font-semibold text-slate-900 dark:text-slate-100">
                  {pillar.title}
                </h2>
                <p className="mt-3 text-sm leading-relaxed text-slate-600 dark:text-slate-300/90">
                  {pillar.description}
                </p>
              </article>
            ))}
          </section>

          <section className="space-y-6">
            <div>
              <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">
                Architecture overview
              </h2>
              <p className="mt-3 text-base text-slate-600 dark:text-slate-300/90">
                A Vite-compiled React SPA runs entirely client-side crypto.
                Users authenticate with Ed25519 signatures while payloads are
                sealed with XChaCha20-Poly1305 before any network hop. Rocket
                enforces policies, persists ciphertext with RocksDB, and streams
                through Fly.io and Tor without observing plaintext. Each
                cryptographic operation receives dual verification: primary
                validation from the Rust crypto engine and secondary
                confirmation from an independent OCaml service using
                mirage-crypto.
              </p>
            </div>
            <div className="w-full">
              <MermaidDiagram
                id="architecture-flow"
                chart={architectureDiagram}
                ariaLabel="System architecture"
                title="Client, edge, and core services"
                description="Trace how sealed payloads travel from the browser to edge POPs and the Rocket core before resting in persistence or relaying to asynchronous workers."
                defaultOpen
              />
            </div>
          </section>

          <section className="space-y-6">
            <div>
              <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">
                Dual Cryptographic Verification
              </h2>
              <p className="mt-3 max-w-3xl text-base text-slate-600 dark:text-slate-300/90">
                copypaste.fyi implements an innovative defense-in-depth approach
                with dual cryptographic verification. Every encryption operation
                is independently validated by two separate implementations: the
                primary Rust crypto engine and a secondary OCaml service using
                mirage-crypto. This ensures that even if one implementation has
                a flaw, the other provides backup verification.
              </p>
            </div>
            <div className="grid gap-6 md:grid-cols-2">
              <div className="rounded-2xl border border-rose-200/70 bg-rose-50/90 p-6 shadow-md dark:border-rose-800/60 dark:bg-rose-900/20">
                <div className="flex items-center space-x-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-rose-100 dark:bg-rose-800/50">
                    <span className="text-lg">🦀</span>
                  </div>
                  <div>
                    <h3 className="font-semibold text-rose-900 dark:text-rose-100">
                      Primary: Rust Engine
                    </h3>
                    <p className="text-sm text-rose-700 dark:text-rose-300">
                      aes-gcm, chacha20poly1305 crates
                    </p>
                  </div>
                </div>
                <p className="mt-4 text-sm text-rose-800 dark:text-rose-200">
                  Fast, battle-tested Rust cryptography with comprehensive
                  security audits and widespread adoption in production systems.
                </p>
              </div>
              <div className="rounded-2xl border border-pink-200/70 bg-pink-50/90 p-6 shadow-md dark:border-pink-800/60 dark:bg-pink-900/20">
                <div className="flex items-center space-x-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-pink-100 dark:bg-pink-800/50">
                    <span className="text-lg">🐫</span>
                  </div>
                  <div>
                    <h3 className="font-semibold text-pink-900 dark:text-pink-100">
                      Secondary: OCaml Service
                    </h3>
                    <p className="text-sm text-pink-700 dark:text-pink-300">
                      mirage-crypto library
                    </p>
                  </div>
                </div>
                <p className="mt-4 text-sm text-pink-800 dark:text-pink-200">
                  Independent verification using OCaml's strong type system and
                  formal methods, providing mathematical assurance of
                  correctness.
                </p>
              </div>
            </div>
            <div className="rounded-xl border border-slate-200/70 bg-slate-50/90 p-6 dark:border-slate-700/60 dark:bg-slate-900/70">
              <h4 className="font-semibold text-slate-900 dark:text-slate-100 mb-2">
                Defense-in-Depth Benefits
              </h4>
              <ul className="space-y-2 text-sm text-slate-700 dark:text-slate-300">
                <li className="flex items-start space-x-2">
                  <span className="text-green-600 dark:text-green-400 mt-1">
                    ✓
                  </span>
                  <span>
                    Independent validation prevents single-point crypto failures
                  </span>
                </li>
                <li className="flex items-start space-x-2">
                  <span className="text-green-600 dark:text-green-400 mt-1">
                    ✓
                  </span>
                  <span>
                    Different languages and libraries reduce common-mode
                    failures
                  </span>
                </li>
                <li className="flex items-start space-x-2">
                  <span className="text-green-600 dark:text-green-400 mt-1">
                    ✓
                  </span>
                  <span>
                    Mathematical verification through OCaml's type system
                  </span>
                </li>
                <li className="flex items-start space-x-2">
                  <span className="text-green-600 dark:text-green-400 mt-1">
                    ✓
                  </span>
                  <span>
                    Transparent operation - no impact on user experience
                  </span>
                </li>
              </ul>
            </div>
          </section>

          <section className="space-y-6">
            <div>
              <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">
                Advanced Cryptographic Techniques
              </h2>
              <p className="mt-3 max-w-3xl text-base text-slate-600 dark:text-slate-300/90">
                Beyond traditional encryption, copypaste.fyi explores
                cutting-edge cryptographic primitives that could enhance
                security, privacy, and functionality for sensitive data sharing.
                These techniques represent the future of cryptographic
                engineering.
              </p>
            </div>
            <div className="grid gap-6 md:grid-cols-3">
              <div className="rounded-2xl border border-indigo-200/70 bg-indigo-50/90 p-6 shadow-md dark:border-indigo-800/60 dark:bg-indigo-900/20">
                <div className="flex items-center space-x-3 mb-4">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-indigo-100 dark:bg-indigo-800/50">
                    <span className="text-lg">🔐</span>
                  </div>
                  <h3 className="font-semibold text-indigo-900 dark:text-indigo-100">
                    Post-Quantum Hybrid
                  </h3>
                </div>
                <p className="text-sm text-indigo-800 dark:text-indigo-200 mb-3">
                  Kyber KEM + AES-256-GCM hybrid encryption providing
                  quantum-resistant key exchange with proven symmetric
                  encryption.
                </p>
                <div className="text-xs text-indigo-700 dark:text-indigo-300">
                  <strong>Status:</strong> Implemented - Ready for production
                  use
                </div>
              </div>

              <div className="rounded-2xl border border-purple-200/70 bg-purple-50/90 p-6 shadow-md dark:border-purple-800/60 dark:bg-purple-900/20">
                <div className="flex items-center space-x-3 mb-4">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-purple-100 dark:bg-purple-800/50">
                    <span className="text-lg">🎭</span>
                  </div>
                  <h3 className="font-semibold text-purple-900 dark:text-purple-100">
                    Zero-Knowledge Proofs
                  </h3>
                </div>
                <p className="text-sm text-purple-800 dark:text-purple-200 mb-3">
                  Prove properties about encrypted data without revealing the
                  underlying content.
                </p>
                <div className="text-xs text-purple-700 dark:text-purple-300">
                  <strong>Use case:</strong> Verify paste properties (length,
                  format) without decryption
                </div>
              </div>

              <div className="rounded-2xl border border-teal-200/70 bg-teal-50/90 p-6 shadow-md dark:border-teal-800/60 dark:bg-teal-900/20">
                <div className="flex items-center space-x-3 mb-4">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-teal-100 dark:bg-teal-800/50">
                    <span className="text-lg">🔄</span>
                  </div>
                  <h3 className="font-semibold text-teal-900 dark:text-teal-100">
                    Homomorphic Encryption
                  </h3>
                </div>
                <p className="text-sm text-teal-800 dark:text-teal-200 mb-3">
                  Perform computations on encrypted data without decryption.
                </p>
                <div className="text-xs text-teal-700 dark:text-teal-300">
                  <strong>Potential:</strong> Search encrypted pastes or apply
                  transformations
                </div>
              </div>

              <div className="rounded-2xl border border-orange-200/70 bg-orange-50/90 p-6 shadow-md dark:border-orange-800/60 dark:bg-orange-900/20">
                <div className="flex items-center space-x-3 mb-4">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-orange-100 dark:bg-orange-800/50">
                    <span className="text-lg">👥</span>
                  </div>
                  <h3 className="font-semibold text-orange-900 dark:text-orange-100">
                    Threshold Cryptography
                  </h3>
                </div>
                <p className="text-sm text-orange-800 dark:text-orange-200 mb-3">
                  Split cryptographic keys across multiple parties for enhanced
                  security.
                </p>
                <div className="text-xs text-orange-700 dark:text-orange-300">
                  <strong>Application:</strong> Multi-signature access control
                  for sensitive pastes
                </div>
              </div>

              <div className="rounded-2xl border border-cyan-200/70 bg-cyan-50/90 p-6 shadow-md dark:border-cyan-800/60 dark:bg-cyan-900/20">
                <div className="flex items-center space-x-3 mb-4">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-cyan-100 dark:bg-cyan-800/50">
                    <span className="text-lg">⏰</span>
                  </div>
                  <h3 className="font-semibold text-cyan-900 dark:text-cyan-100">
                    Verifiable Delay Functions
                  </h3>
                </div>
                <p className="text-sm text-cyan-800 dark:text-cyan-200 mb-3">
                  Cryptographic proof of time passage without trusted
                  timestamps.
                </p>
                <div className="text-xs text-cyan-700 dark:text-cyan-300">
                  <strong>Benefit:</strong> Decentralized time-lock encryption
                  for scheduled reveals
                </div>
              </div>

              <div className="rounded-2xl border border-green-200/70 bg-green-50/90 p-6 shadow-md dark:border-green-800/60 dark:bg-green-900/20">
                <div className="flex items-center space-x-3 mb-4">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-green-100 dark:bg-green-800/50">
                    <span className="text-lg">🔗</span>
                  </div>
                  <h3 className="font-semibold text-green-900 dark:text-green-100">
                    Secure Multi-Party Computation
                  </h3>
                </div>
                <p className="text-sm text-green-800 dark:text-green-200 mb-3">
                  Multiple parties compute functions on private inputs without
                  revealing data.
                </p>
                <div className="text-xs text-green-700 dark:text-green-300">
                  <strong>Future:</strong> Collaborative analysis of encrypted
                  datasets
                </div>
              </div>
            </div>

            <div className="rounded-xl border border-slate-200/70 bg-gradient-to-r from-slate-50/90 to-slate-100/90 p-6 dark:border-slate-700/60 dark:from-slate-900/70 dark:to-slate-800/70">
              <h4 className="font-semibold text-slate-900 dark:text-slate-100 mb-3">
                Why These Techniques Matter
              </h4>
              <div className="grid gap-4 md:grid-cols-2">
                <div>
                  <h5 className="font-medium text-slate-800 dark:text-slate-200 mb-2">
                    🔮 Future-Proofing
                  </h5>
                  <p className="text-sm text-slate-700 dark:text-slate-300">
                    As quantum computing advances and privacy needs evolve,
                    these techniques provide building blocks for next-generation
                    secure data sharing.
                  </p>
                </div>
                <div>
                  <h5 className="font-medium text-slate-800 dark:text-slate-200 mb-2">
                    🏗️ Research Foundation
                  </h5>
                  <p className="text-sm text-slate-700 dark:text-slate-300">
                    copypaste.fyi serves as a platform for exploring practical
                    applications of advanced cryptography in real-world
                    scenarios.
                  </p>
                </div>
              </div>
            </div>
          </section>

          <section className="space-y-6">
            <div>
              <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">
                Lifecycle of a paste
              </h2>
              <p className="mt-3 max-w-3xl text-base text-slate-600 dark:text-slate-300/90">
                Every submission is cryptographically self-contained: the
                browser derives ephemeral keys, signs the payload, and ships
                sealed content to the API. Rocket verifies every step before
                persistence or streaming back to the caller.
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
              <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">
                Policy state machine
              </h2>
              <p className="mt-3 max-w-3xl text-base text-slate-600 dark:text-slate-300/90">
                Enforcement is deterministic: once payloads are sealed, Rocket
                promotes them through evaluated states and prevents resurrection
                of burned or expired artifacts—even if the persistence layer is
                tampered with.
              </p>
              <MermaidDiagram
                id="policy-machine"
                chart={policyDiagram}
                ariaLabel="Policy enforcement state machine"
                title="Policy transition graph"
                description="Deterministic state progression driven by burn-after-reading, retention windows, or governance archival."
                defaultOpen={false}
              />
            </div>
          </section>

          <section>
            <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">
              Operational use cases
            </h2>
            <div className="mt-6 grid gap-6 md:grid-cols-2">
              {useCases.map((item) => (
                <article
                  key={item.title}
                  className="rounded-2xl border border-slate-200/70 bg-white/95 p-6 shadow-md transition hover:-translate-y-0.5 hover:shadow-lg dark:border-slate-700/60 dark:bg-slate-900/70"
                >
                  <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
                    {item.title}
                  </h3>
                  <p className="mt-3 text-sm leading-relaxed text-slate-600 dark:text-slate-300/90">
                    {item.detail}
                  </p>
                </article>
              ))}
            </div>
          </section>

          <section>
            <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">
              Roadmap
            </h2>
            <div className="mt-6 grid gap-6 md:grid-cols-2">
              {roadmapItems.map((item) => (
                <article
                  key={item.title}
                  className="rounded-2xl border border-slate-200/70 bg-white/95 p-6 shadow-md transition hover:-translate-y-0.5 hover:shadow-lg dark:border-slate-700/60 dark:bg-slate-900/70"
                >
                  <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
                    {item.title}
                  </h3>
                  <p className="mt-3 text-sm leading-relaxed text-slate-600 dark:text-slate-300/90">
                    {item.detail}
                  </p>
                </article>
              ))}
            </div>
          </section>

          <section className="rounded-3xl border border-white/80 bg-white/85 p-8 shadow-xl backdrop-blur-xl dark:border-white/10 dark:bg-white/5">
            <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100">
              Transparency & verification
            </h2>
            <p className="mt-4 text-base text-slate-600 dark:text-slate-300/90">
              Every repository, deployment manifest, and cryptographic primitive
              is published for review. Threat models, reproducible build
              scripts, and signed releases ship alongside the codebase so
              researchers can validate implementations instead of trusting
              claims.
            </p>
            <p className="mt-4 text-base text-slate-600 dark:text-slate-300/90">
              Responsible disclosure channels provide encrypted communication
              paths. Security advisories are signed with the project key to
              guarantee authenticity, and regression tests codify discovered
              issues to prevent recurrence.
            </p>
            <p className="mt-4 text-base text-slate-600 dark:text-slate-300/90">
              The{" "}
              <span className="font-semibold text-primary">
                Privacy Journey
              </span>{" "}
              indicator (bottom-left corner) shows real-time detection of
              privacy measures protecting your connection—including HTTPS/TLS,
              Tor network, VPN/proxy, Do Not Track headers, private browsing
              mode, and client-side encryption. Inspired by{" "}
              <a
                href="https://how-did-i-get-here.net/"
                target="_blank"
                rel="noopener noreferrer"
                className="font-semibold text-primary hover:underline"
              >
                how-did-i-get-here.net
              </a>
              , this feature educates users about the privacy layers
              safeguarding their data.
            </p>
          </section>
        </main>
      </div>
    </div>
  );
};

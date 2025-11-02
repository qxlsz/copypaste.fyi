<div align="center">

# copypaste.fyi

Simple, open-source paste sharing for teams and individuals.

[![Docker](https://img.shields.io/badge/docker-compose-blue?logo=docker)](#run-with-docker-compose)
[![Rust](https://img.shields.io/badge/rust-1.82+-orange?logo=rust)](#run-locally)

</div>

## Overview

copypaste.fyi is a lightweight web service for creating and sharing plaintext snippets. It focuses on fast paste creation, predictable URLs, and minimal operational overhead. The UI is intentionally simple and responsive, making it easy to share links from any device.

Key traits:

- 🧠 **Zero complexity** – in-memory storage with minimal dependencies.
- ⚡ **Fast** – Rocket-based async backend with Tokio.
- 🐳 **Container friendly** – ready-to-run Docker image and compose service.
- 🔗 **Scriptable** – companion CLI (`cpaste`) for shell automation.
- 🧨 **One-time links** – optional burn-after-reading destroys pastes after the first successful view.

## Architecture

```mermaid
graph TD
    classDef client fill:#2563eb,stroke:#1e3a8a,color:#fff;
    classDef service fill:#10b981,stroke:#047857,color:#fff;
    classDef logic fill:#f59e0b,stroke:#b45309,color:#fff;
    classDef storage fill:#f87171,stroke:#b91c1c,color:#fff;
    classDef util fill:#8b5cf6,stroke:#5b21b6,color:#fff;
    classDef tooling fill:#0ea5e9,stroke:#0369a1,color:#fff;
    classDef link stroke-width:2px;

    subgraph Client Tier
        A[React SPA\nVite dev server]
        B[CLI\n`cpaste` binary]
        C[Server-rendered fallback]
    end

    subgraph Service Layer
        D[Rocket Router\n`GET /`, `GET /p/<id>`]
        E[REST API\n`POST /api/pastes`, `GET /api/pastes/<id>`]
        F[Static assets\n`/static`, SPA fallback]
    end

    subgraph Domain Logic
        G[Encryption & Key Derivation]
        H[Attestation & Time Locks]
        I[Bundle + Burn-after-reading]
        J[Renderers\nMarkdown / code / raw]
        K[Webhooks]
    end

    subgraph Persistence
        L[PasteStore Trait]
        M[MemoryPasteStore\nEphemeral HashMap]
    end

    subgraph Tooling
        N[Vitest + ESLint]
        O[Cargo fmt / clippy / nextest]
    end

    A -->|fetch| E
    B -->|HTTP JSON| E
    C -->|HTML view| D
    D --> F
    D --> G
    D --> H
    E --> G
    E --> H
    G --> L
    H --> L
    I --> L
    L --> M
    D --> J
    E --> I
    I --> K
    N --> A
    O --> D

    class A,B,C client;
    class D,E,F service;
    class G,H,I,J,K logic;
    class L,M storage;
    class N,O tooling;
    class A,B,C,D,E,F,G,H,I,J,K,L,M,N,O link;
```

The SPA communicates with the Rocket REST API for creation and viewing, while the server still renders HTML for raw links and one-time fallbacks. Domain helpers handle encryption, attestations, bundles, and webhook notifications before persisting to the in-memory store.

- **Backend:** Rust (edition 2021), Rocket 0.5, Tokio 1.x
- **Frontend:** React 19 + Vite 7, TanStack Query, Tailwind CSS
- **Storage:** Ephemeral in-memory `PasteStore`
- **CLI:** `cpaste` using `reqwest`
- **Tooling:** Cargo fmt/clippy/nextest, Vitest, ESLint

## Getting Started

### Prerequisites

- Rust toolchain (1.82+) installed via [rustup](https://rustup.rs/) – for local builds
- Docker (24+) and Docker Compose v2 – for containerized setup

### Repository setup

Clone the repository, then install the tooling and git hooks used by CI:

```bash
# Install rustup toolchain, fmt/clippy, cargo-nextest, cargo-llvm-cov
./scripts/install_deps.sh

# Install the pre-commit hook (runs fmt, clippy, nextest on every commit)
./scripts/setup_git_hooks.sh
```

If the hook rewrites files (via `cargo fmt`) or a check fails, the commit is aborted so you can address the issue and re-stage. You can always run the steps manually with `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo nextest run --workspace --all-features`.

### Run Locally

```bash
# Fetch dependencies and build
cargo build

# Start the web server
cargo run --bin copypaste

# Application available at http://127.0.0.1:8000/
```

Once running, open a browser to `http://127.0.0.1:8000/`, enter text, and hit **Create paste** to receive a link. The backend serves the pre-built SPA from `static/dist` when the Vite dev server is not running.

### Frontend development

```bash
cd frontend
npm install               # run once; scripts/precommit.sh handles this too
npm run dev               # Vite dev server at http://127.0.0.1:5173/

# Lint + unit tests (Vitest)
npm run lint
npm test -- --run

# Build production assets (writes to static/dist)
npm run build
```

For an all-in-one local environment (Rocket API + Vite dev server) run `./scripts/run_both.sh`. Stop everything with `./scripts/stop.sh`.

## REST API

Interact with copypaste.fyi programmatically through the JSON API. All endpoints live under `/api` and accept/return UTF-8 JSON.

### Create a paste

`POST /api/pastes`

```bash
curl -X POST http://127.0.0.1:8000/api/pastes \
  -H 'Content-Type: application/json' \
  -d '{
        "content": "Hello from the API",
        "format": "plain_text",
        "retention_minutes": 60,
        "burn_after_reading": false,
        "encryption": {
          "algorithm": "aes256_gcm",
          "key": "correct-horse-battery-staple"
        }
      }'
```

**Request body**

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `content` | `string` | ✅ | Paste body. |
| `format` | `string` | ❌ | One of `plain_text`, `markdown`, `code`, `json`, `go`, `cpp`, `kotlin`, `java`. Defaults to `plain_text`. |
| `retention_minutes` | `number` | ❌ | Minutes before automatic deletion. Omit for no expiry. |
| `burn_after_reading` | `boolean` | ❌ | Delete paste after first successful view. |
| `encryption.algorithm` | `string` | ❌ | `aes256_gcm`, `chacha20_poly1305`, or `xchacha20_poly1305`. |
| `encryption.key` | `string` | ⚠️ | Required when `encryption.algorithm` is provided. Never stored server-side. |

**Response**

```jsonc
{
  "id": "AbCdEf12",
  "shareableUrl": "/p/AbCdEf12",
  "burnAfterReading": false
}
```

The `shareableUrl` is relative to the server origin. For encrypted pastes, append `?key=<secret>` to the share link before sharing.

### Fetch a paste

`GET /api/pastes/{id}`

```bash
curl http://127.0.0.1:8000/api/pastes/AbCdEf12

# Encrypted paste
curl "http://127.0.0.1:8000/api/pastes/AbCdEf12?key=correct-horse-battery-staple"
```

**Response**

```jsonc
{
  "id": "AbCdEf12",
  "content": "Hello from the API",
  "format": "plain_text",
  "createdAt": 1730518840,
  "expiresAt": null,
  "burnAfterReading": false,
  "encryption": {
    "requiresKey": true,
    "algorithm": "aes256_gcm"
  },
  "timeLock": null,
  "persistence": {
    "kind": "memory"
  }
}
```

A `401 Unauthorized` response indicates a missing or invalid key for an encrypted paste. A `404` means the paste never existed or was already burned/time-locked.

### Raw paste view

`GET /p/{id}/raw`

Returns plain text (no JSON envelope) for shell-friendly consumption.

```bash
curl http://127.0.0.1:8000/p/AbCdEf12/raw
```

Encrypted pastes require the key query parameter: `/p/{id}/raw?key=<secret>`.

> 💡 Looking for CLI automation? See [CLI Usage (`cpaste`)](#cli-usage-cpaste) for examples that wrap these endpoints.

### Formatting options

- Plain text / Markdown / generic code block
- Language-specific code blocks: Go, C++, Kotlin, Java
- JSON pretty-print (parses and auto-indents or shows raw fallback)

**Encryption options**

- `None` – store plaintext (default)
- `AES-256-GCM` – deterministic 12-byte nonce per paste, client-supplied passphrase
- `ChaCha20-Poly1305` – compact 96-bit nonce cipher for performance-oriented clients
- `XChaCha20-Poly1305` – 24-byte nonce variant suited for longer keys and high-entropy secrets

The web UI includes multiple passphrase helpers (**Geek**, **Emoji combo**, **Diceware blend**) and a live key-strength meter. Keys stay visible (or toggle to hidden) so you can share them out-of-band—the server never stores them. A share panel provides easy copy, email, Slack, X/Twitter, QR, and native share shortcuts.

**Extras**

- Burn after reading: toggle in the composer (or pass `--burn-after-reading` via CLI) to delete the paste after the first successful view.
- Raw view: append `/raw/<id>` (plus `?key=<passphrase>` when encrypted) to retrieve plaintext without HTML chrome.

➡️ Dive deeper in the [Encryption guide](docs/encryption.md) for algorithm notes, key derivation details, and operational advice.

### Run with Docker Compose

```bash
docker compose up --build

# Visit http://127.0.0.1:8000/
```

Compose mounts the `static/` directory for live UI updates. Data is stored in-memory inside the container; restart clears pastes.

### CLI Usage (`cpaste`)

Build the standalone CLI and point it at any copypaste.fyi instance.

```bash
# Build the binary
cargo build --bin cpaste --release

# Send text directly (defaults to http://127.0.0.1:8000)
./target/release/cpaste -- "Hello from CLI"

# Switch hosts as needed
./target/release/cpaste --host https://copypaste.fyi -- "notes"

# Stream from stdin
echo "log output" | ./target/release/cpaste --stdin --host http://localhost:8000 --
```

**Flags & arguments**

| Option | Description |
| ------ | ----------- |
| `--host <URL>` | Base URL of the copypaste server. Defaults to `http://127.0.0.1:8000`. |
| `--stdin` | Read the paste content from standard input instead of the command line argument. |
| `--format <plain_text|markdown|code|json|go|cpp|kotlin|java>` | Rendering mode for the paste. Defaults to `plain_text`. |
| `--encryption <none|aes256_gcm|chacha20_poly1305|xchacha20_poly1305>` | Client-side encryption algorithm. When not `none`, pass `--key`. |
| `--key <string>` | Encryption key / passphrase (required for encrypted pastes). |
| `--burn-after-reading` | Delete the paste immediately after the first successful view (one-time link). |
| positional text | When `--stdin` is not provided, supply the text to paste as a positional argument. |

`cpaste --help` displays the full command reference.

### Packaging CLI for Releases

The repository includes a helper to bundle the CLI binary for GitHub releases.

```
# Build and package version 0.2.0 under dist/
./scripts/package_cli.sh 0.2.0

# Artifacts created:
# - dist/cpaste-0.2.0.tar.gz
# - dist/cpaste-0.2.0.tar.gz.sha256

# Suggested workflow:
# 1. git tag -a v0.2.0 -m "Release v0.2.0"
# 2. git push origin v0.2.0
# 3. Draft a GitHub release and upload the tarball + checksum
```

A GitHub Actions workflow (`.github/workflows/release.yml`) automates steps 2–4 whenever a tag matching `v*` is pushed: it runs the packaging script and publishes the generated artifacts as release assets.

## Project Structure

```
copypaste.fyi/
├── Cargo.toml          # Rust workspace and dependencies
├── Dockerfile          # Multi-stage build for production images
├── docker-compose.yml  # Local orchestration
├── src/
│   ├── lib.rs          # PasteStore trait + memory implementation
│   ├── main.rs         # Rocket application entry point
│   └── bin/
│       └── cpaste.rs   # CLI client
└── static/
    └── index.html      # Frontend interface
```

## Development Notes

- Pastes are kept in-process; production deployments should consider persistent storage.
- Use `cargo fmt` and `cargo clippy` before committing.
- The Docker image is built with Rust 1.82 slim base and serves the compiled binary on Debian bookworm.
- Async code steers clear of the futurelock pattern described in [RFD 609](https://rfd.shared.oxide.computer/rfd/0609); when adding concurrent flows, prefer spawning owned futures or a `JoinSet` instead of holding borrowed futures across `await` points.

## Contributing

Pull requests are welcome! Please:

1. Install the tooling and git hooks described in [Repository setup](#repository-setup).
2. Ensure formatting, linting, and tests pass locally: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo nextest run --workspace --all-features`.
3. (Optional but encouraged) Verify coverage meets CI expectations: `cargo llvm-cov --workspace --all-features --nextest --fail-under-lines 75`.
4. Keep changes focused and add tests when extending functionality.

## License

Licensed under the terms of the [MIT License](LICENSE).



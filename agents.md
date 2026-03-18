# Agents Guide: copypaste.fyi

This document provides comprehensive information for AI agents working on this codebase.

## Project Overview

**copypaste.fyi** is a lightweight, open-source paste sharing service designed for teams and individuals. It's a full-stack web application with:

- **Backend**: Rust with Rocket web framework (v0.5.0-rc.3)
- **Frontend**: React 19 with Vite 7 and TypeScript
- **Cryptographic Verification**: OCaml service using `mirage-crypto`
- **Blockchain Integration**: Hardhat/Solidity for optional paste anchoring

**Key Purpose**: Fast, secure, privacy-focused paste sharing with client-side encryption, burn-after-reading links, and advanced security features like post-quantum cryptography.

---

## Directory Structure

```
copypaste.fyi/
├── src/                          # Rust backend source code
│   ├── main.rs                   # Entry point (calls handlers::launch())
│   ├── lib.rs                    # Core types, trait definitions
│   ├── bin/
│   │   └── cpaste.rs             # CLI client for automated paste creation
│   └── server/                   # Server modules
│       ├── handlers.rs           # HTTP request handlers & route definitions
│       ├── models.rs             # Request/response DTOs
│       ├── crypto.rs             # Encryption/decryption (AES-256-GCM, ChaCha20, Kyber)
│       ├── render.rs             # HTML rendering for paste views
│       ├── attestation.rs        # TOTP & shared secret verification
│       ├── blockchain.rs         # Paste anchoring to blockchain
│       ├── redis.rs              # Optional Redis persistence adapter
│       ├── webhook.rs            # Webhook event triggers (Slack/Teams/Generic)
│       ├── bundles.rs            # Multi-share bundle functionality
│       ├── tor.rs                # Tor onion service configuration
│       ├── stego.rs              # Steganography: embed pastes in images
│       ├── time.rs               # Time-lock & retention logic
│       └── cors.rs               # CORS middleware
├── frontend/                     # React/Vite SPA
│   ├── src/
│   │   ├── main.tsx              # React entry point
│   │   ├── App.tsx               # Router setup & layout
│   │   ├── pages/
│   │   │   ├── PasteForm.tsx     # Main compose/create UI
│   │   │   ├── PasteView.tsx     # Display & decrypt pastes
│   │   │   ├── Dashboard.tsx     # User paste management
│   │   │   ├── Stats.tsx         # Usage statistics
│   │   │   ├── Login.tsx         # Authentication
│   │   │   ├── About.tsx         # Information page
│   │   │   └── UserPastes.tsx    # User's paste list
│   │   ├── components/
│   │   │   ├── PrivacyJourney.tsx # Privacy indicator widget
│   │   │   ├── Layout.tsx        # Main layout wrapper
│   │   │   ├── CommandPalette.tsx # Keyboard shortcuts
│   │   │   ├── editor/           # Monaco editor integration
│   │   │   ├── charts/           # D3 visualization components
│   │   │   └── ui/               # Reusable UI components
│   │   ├── stores/
│   │   │   ├── pasteComposer.ts  # Zustand store for form state
│   │   │   └── auth.ts           # Authentication state
│   │   ├── api/
│   │   │   ├── client.ts         # REST API client
│   │   │   └── types.ts          # TypeScript types
│   │   └── theme/                # Theme configuration
│   ├── vite.config.ts            # Vite dev server proxy & build config
│   └── package.json              # Dependencies
├── ocaml-crypto-verifier/        # OCaml cryptographic verification service
│   ├── lib/crypto_verifier.ml    # Core verification logic
│   ├── bin/server.ml             # HTTP server (port 8001)
│   └── Dockerfile                # Multi-stage build
├── blockchain/                   # Hardhat/Solidity project
│   ├── contracts/PasteAnchor.sol # Smart contract for paste anchoring
│   ├── scripts/deploy.ts         # Deployment script
│   └── hardhat.config.ts         # Hardhat configuration
├── static/                       # Static assets & dist
│   ├── index.html                # SPA entry point
│   ├── view.css                  # Stylesheet for paste view fallback
│   └── dist/                     # Built frontend (from Vite build)
├── tests/                        # Integration tests
│   ├── lib_tests.rs              # Library functionality tests
│   └── crypto_tests.rs           # Encryption tests
├── scripts/                      # Build & development scripts
│   ├── run_both.sh               # Start Rocket + Vite dev server
│   ├── run_backend.sh            # Start Rocket only
│   ├── run_frontend.sh           # Start Vite only
│   ├── package_cli.sh            # Package CLI for releases
│   ├── install_deps.sh           # Install build dependencies
│   ├── setup_git_hooks.sh        # Install pre-commit hook
│   └── precommit.sh              # Pre-commit checks (fmt, clippy, nextest)
├── docs/                         # Documentation
│   └── encryption.md             # Encryption algorithm details
├── .github/workflows/            # CI/CD pipelines
│   ├── ci.yml                    # Main test suite
│   ├── deploy-fly.yml            # Fly.io deployment
│   ├── ocaml-ci.yml              # OCaml service tests
│   └── release.yml               # GitHub release automation
├── Cargo.toml                    # Rust dependencies
├── Dockerfile                    # Multi-stage production image
├── Dockerfile.backend            # Backend-only Dockerfile for Fly.io
├── docker-compose.yml            # Local orchestration
├── fly.toml                      # Fly.io deployment configuration
└── vercel.json                   # Vercel (frontend only) deployment
```

---

## How to Run

### Development Setup

```bash
# 1. Install dependencies
./scripts/install_deps.sh              # Rust tools, cargo-nextest, cargo-llvm-cov
./scripts/setup_git_hooks.sh           # Pre-commit hook

# 2. Start backend and frontend together
./scripts/run_both.sh                  # Rocket @ :8000 + Vite @ :5173

# OR start individually:
./scripts/run_backend.sh               # Rocket only
cd frontend && npm run dev             # Vite only

# 3. Stop services
./scripts/stop.sh
```

### Building

```bash
# Rust backend
cargo build --release
cargo build --bin cpaste --release    # CLI tool

# Frontend
cd frontend
npm install
npm run build                          # Outputs to static/dist

# Full Docker build
docker compose up --build
```

### Testing

```bash
# Rust tests
cargo nextest run --workspace --all-features
cargo fmt --all && cargo clippy --all-targets --all-features

# Frontend tests
cd frontend
npm test -- --run                      # Vitest
npm run lint                           # ESLint
npm run test:e2e                       # Playwright (E2E)
npm run test:health                    # Health check E2E

# Coverage
cargo llvm-cov nextest --workspace --all-features --fail-under-lines 75
```

### Docker

```bash
# Development
docker compose up --build

# Production (Fly.io)
fly deploy
fly logs

# Custom deployment
docker build -f Dockerfile.backend -t copypaste-backend .
docker run -p 8000:8000 -p 8001:8001 copypaste-backend
```

### CLI Usage (cpaste)

```bash
# Build CLI
cargo build --bin cpaste --release

# Basic usage
./target/release/cpaste -- "Hello from CLI"

# With encryption
./target/release/cpaste --encryption aes256_gcm --key "my-passphrase" -- "secret"

# From stdin
echo "log output" | ./target/release/cpaste --stdin --host http://localhost:8000 --

# Advanced
./target/release/cpaste --format markdown --retention 60 --burn-after-reading -- "content"
```

---

## Backend Components (Rust)

| Module | File | Purpose |
|--------|------|---------|
| **handlers** | `src/server/handlers.rs` | HTTP route handlers: POST /api/pastes, GET /api/pastes/{id}, /p/{id} |
| **crypto** | `src/server/crypto.rs` | Encryption: AES-256-GCM, ChaCha20-Poly1305, XChaCha20, Kyber hybrid |
| **render** | `src/server/render.rs` | HTML rendering for paste views, markdown/JSON/code formatting |
| **attestation** | `src/server/attestation.rs` | TOTP and shared secret verification |
| **webhook** | `src/server/webhook.rs` | Async webhook dispatch for Slack, Teams, or generic HTTP |
| **blockchain** | `src/server/blockchain.rs` | Paste manifest anchoring and anchor receipt generation |
| **redis** | `src/server/redis.rs` | Persistence adapter for Redis (optional) |
| **bundles** | `src/server/bundles.rs` | Multi-share bundles with independent burns |
| **tor** | `src/server/tor.rs` | Tor onion service configuration |
| **stego** | `src/server/stego.rs` | Steganography: embed encrypted pastes in PNG images |
| **time** | `src/server/time.rs` | Time-lock evaluation (not_before, not_after), retention/expiry |
| **models** | `src/server/models.rs` | Request/response DTOs |
| **cors** | `src/server/cors.rs` | CORS middleware |

---

## Frontend Components (React)

### Pages

| Page | File | Purpose |
|------|------|---------|
| **PasteForm** | `frontend/src/pages/PasteForm.tsx` | Compose interface: editor, format, encryption options |
| **PasteView** | `frontend/src/pages/PasteView.tsx` | Display/decrypt pastes, render formatted content |
| **Dashboard** | `frontend/src/pages/Dashboard.tsx` | User dashboard: analytics, recent pastes |
| **Stats** | `frontend/src/pages/Stats.tsx` | Global statistics: format distribution, encryption usage |
| **Login** | `frontend/src/pages/Login.tsx` | Authentication entry |
| **About** | `frontend/src/pages/About.tsx` | Feature documentation, privacy info |
| **UserPastes** | `frontend/src/pages/UserPastes.tsx` | User's paste history |

### Key Components

| Component | File | Purpose |
|-----------|------|---------|
| **PrivacyJourney** | `frontend/src/components/PrivacyJourney.tsx` | Real-time privacy status: HTTPS, Tor, VPN, DNT |
| **Layout** | `frontend/src/components/Layout.tsx` | Main page layout wrapper |
| **CommandPalette** | `frontend/src/components/CommandPalette.tsx` | Keyboard shortcuts |
| **MonacoEditor** | `frontend/src/components/editor/` | Rich code editor with syntax highlighting |
| **Charts** | `frontend/src/components/charts/` | D3-based visualizations |

### Stores (Zustand)

| Store | File | Purpose |
|-------|------|---------|
| **pasteComposer** | `frontend/src/stores/pasteComposer.ts` | Form state management |
| **auth** | `frontend/src/stores/auth.ts` | Authentication state |

---

## OCaml Crypto Verifier

- **Port**: 8001
- **Files**: `ocaml-crypto-verifier/`
- **Endpoints**:
  - `GET /health` - health check
  - `POST /verify/encryption` - verify encryption operations
  - `POST /verify/signature` - verify Ed25519 signatures
- **Purpose**: Independent cryptographic verification as defense-in-depth

---

## API Endpoints

### Create Paste
```
POST /api/pastes
Content-Type: application/json

{
  "content": "text",
  "format": "plain_text|markdown|code|json|go|cpp|...",
  "retention_minutes": 60,
  "burn_after_reading": false,
  "encryption": {
    "algorithm": "none|aes256_gcm|chacha20_poly1305|xchacha20_poly1305|kyber_hybrid_aes256_gcm",
    "key": "secret"
  }
}

Response: { "id": "...", "shareableUrl": "/p/..." }
```

### Fetch Paste
```
GET /api/pastes/{id}?key=secret

Response: {
  "id": "...",
  "content": "...",
  "format": "...",
  "createdAt": 1234567890,
  "encryption": { "requiresKey": true, "algorithm": "..." },
  "timeLock": null,
  "persistence": { "kind": "memory" }
}
```

### View Paste (HTML)
```
GET /p/{id}
GET /p/{id}?key=secret
Returns: HTML page with rendered paste
```

### Raw Paste
```
GET /p/{id}/raw
GET /p/{id}/raw?key=secret
Returns: Plain text (no JSON wrapper)
```

### Health Check
```
GET /health
GET /api/health
```

### Other Endpoints
- `GET /` - SPA index
- `GET /about.txt` - Plain text service info
- `POST /api/anchor` - Blockchain anchoring
- `POST /api/authenticate` - Auth challenge/login
- `GET /api/stats` - Global statistics

---

## Environment Variables

### Backend (Rust)

| Variable | Purpose | Default | Required When |
|----------|---------|---------|---------------|
| **Rocket Server** |
| `ROCKET_ADDRESS` | Bind address | 0.0.0.0 | No |
| `ROCKET_PORT` | Bind port | 8000 | No |
| `RUST_LOG` | Log level (trace, debug, info, warn, error) | info | No |
| **Persistence Backend** |
| `COPYPASTE_PERSISTENCE_BACKEND` | Storage backend (memory, redis, vault) | memory | No |
| **Redis Backend** (required when `COPYPASTE_PERSISTENCE_BACKEND=redis`) |
| `UPSTASH_REDIS_REST_URL` | Upstash Redis REST API URL | - | **Yes** (if backend=redis) |
| `UPSTASH_REDIS_REST_TOKEN` | Upstash Redis auth token | - | **Yes** (if backend=redis) |
| `COPYPASTE_REDIS_KEY_PREFIX` | Redis key namespace prefix | paste: | No |
| **Vault Backend** (required when `COPYPASTE_PERSISTENCE_BACKEND=vault`) |
| `COPYPASTE_VAULT_ADDR` | Vault server URL (http:// or https://) | - | **Yes** (if backend=vault) |
| `COPYPASTE_VAULT_TOKEN` | Vault auth token (min 20 chars) | - | **Yes** (if backend=vault) |
| `COPYPASTE_VAULT_MOUNT` | KV secrets engine mount point | secret | No |
| `COPYPASTE_VAULT_NAMESPACE` | Vault Enterprise namespace | - | No |
| `COPYPASTE_VAULT_PREFIX` | Key path prefix within mount | copypaste | No |
| **Blockchain Anchoring** (optional) |
| `ANCHOR_RELAY_ENDPOINT` | Blockchain relay service URL | - | No |
| `ANCHOR_RELAY_API_KEY` | Relay service API key | - | No |
| **Tor/Onion Service** (optional) |
| `COPYPASTE_ONION_HOST` | Tor onion service hostname | - | No |
| `COPYPASTE_TOR_SUPPRESS_LOGS` | Suppress Tor-related log messages | true | No |
| **Crypto Verification** |
| `CRYPTO_VERIFIER_URL` | OCaml service endpoint for dual verification | http://localhost:8001 | No |
| **Build Info** (set automatically in Docker builds) |
| `GIT_COMMIT` | Git commit SHA | - | No |
| `GIT_COMMIT_MESSAGE` | Git commit message | - | No |
| `COPYPASTE_VERSION` | Application version | - | No |

### Boolean Values

The following environment variables accept boolean values:

| Variable | Accepted Values |
|----------|-----------------|
| `COPYPASTE_TOR_SUPPRESS_LOGS` | `1`, `true`, `yes`, `on` (enable) / `0`, `false`, `no`, `off` (disable) |

### Frontend (React)

| Variable | Purpose |
|----------|---------|
| `VITE_API_BASE` | API base URL (production only) |

### OCaml Crypto Verifier

| Variable | Purpose | Default |
|----------|---------|---------|
| `PORT` | Listen port | 8001 |
| `HOST` | Bind address | 0.0.0.0 |

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Client Tier                                             │
├─────────────────────────────────────────────────────────┤
│ React SPA (Vite)     │ CLI (cpaste)  │ Server fallback │
└──────────┬───────────────────────────┬──────────────────┘
           │                           │
┌──────────▼──────────────────────────▼──────────────────┐
│ Rocket REST API Layer                                  │
├───────────────────────────────────────────────────────┤
│ POST /api/pastes │ GET /api/pastes/{id} │ /p/{id}/raw │
└──────────┬─────────────────────────────────────────────┘
           │
┌──────────▼──────────────────────────────────────────────┐
│ Domain Logic                                           │
├───────────────────────────────────────────────────────┤
│ Encryption  │ Attestation  │ Bundles  │ Time-locks    │
│ Webhooks    │ Anchoring    │ Stego    │ Renderers     │
└──────────┬────────────────────────────────────────────┘
           │
┌──────────▼──────────────────────────────────────────────┐
│ Storage & Verification                                 │
├───────────────────────────────────────────────────────┤
│ PasteStore trait (in-memory HashMap, optional Redis) │
│ OCaml crypto verifier (independent validation)        │
└───────────────────────────────────────────────────────┘
```

---

## Key Design Patterns

### 1. Trait-Based Persistence
- `PasteStore` trait allows swappable backends (in-memory HashMap, Redis)
- `RedisPersistenceAdapter` implements async Redis integration
- See: `src/lib.rs`, `src/server/redis.rs`

### 2. Client-Side Encryption
- Keys never stored server-side
- Encryption happens in browser before transmission
- Encrypted pastes require out-of-band key sharing

### 3. Post-Quantum Ready
- Kyber hybrid encryption: Kyber KEM + AES-256-GCM
- See: `src/server/crypto.rs`

### 4. Defense-in-Depth
- Dual cryptographic verification: Rust + OCaml service
- Each operation verified by both implementations

### 5. Zero-Complexity Focus
- In-memory storage by default (no database required)
- Minimal dependencies for easy deployment
- Single binary for backend

---

## Security Features

| Feature | Description | Location |
|---------|-------------|----------|
| **Encryption** | AES-256-GCM, ChaCha20, XChaCha20, Kyber hybrid | `src/server/crypto.rs` |
| **Burn-after-reading** | Automatic deletion after first view | `src/server/handlers.rs` |
| **Time-locks** | Schedule paste availability | `src/server/time.rs` |
| **Attestation** | TOTP and shared secrets for access | `src/server/attestation.rs` |
| **HTML Escaping** | XSS prevention | `src/server/render.rs` |
| **Tor Support** | Optional .onion host | `src/server/tor.rs` |
| **Steganography** | Hide content in images | `src/server/stego.rs` |

---

## Dependencies

### Rust (Key Crates)
- **Web**: rocket (0.5.0-rc.3), tokio (1.x)
- **Crypto**: aes-gcm, chacha20poly1305, pqc_kyber, sha2, ed25519-dalek
- **Serialization**: serde, serde_json, utoipa (OpenAPI)
- **Utilities**: reqwest, nanoid, chrono, html-escape, rand, base64

### Frontend (Key Packages)
- **Core**: react (19), react-router-dom (7), vite (7)
- **State**: zustand, @tanstack/react-query (5)
- **UI**: tailwindcss, lucide-react
- **Editor**: @monaco-editor/react
- **Visualization**: d3, mermaid
- **Testing**: vitest, playwright, msw

---

## CI/CD Pipeline

### Main CI (.github/workflows/ci.yml)
1. Formatting: `cargo fmt --all -- --check`
2. Linting: `cargo clippy --all-targets --all-features -- -D warnings`
3. Build: `cargo build --release --all-targets`
4. Tests: `cargo nextest run --workspace --all-features`
5. Frontend tests: `npm test -- --run`
6. Frontend lint: `npm run lint`
7. Coverage: `cargo llvm-cov nextest --fail-under-lines 75`

### Pre-commit Hook
The pre-commit hook runs: `cargo fmt`, `cargo clippy`, `cargo nextest run`

Setup: `./scripts/setup_git_hooks.sh`

---

## Common Tasks for Agents

### Adding a New API Endpoint
1. Add route handler in `src/server/handlers.rs`
2. Add request/response models in `src/server/models.rs`
3. Register route in the `rocket()` function in `handlers.rs`
4. Add frontend API call in `frontend/src/api/client.ts`

### Adding a New Frontend Page
1. Create page component in `frontend/src/pages/`
2. Add route in `frontend/src/App.tsx`
3. Add navigation link if needed in Layout

### Adding a New Encryption Algorithm
1. Add algorithm to enum in `src/server/crypto.rs`
2. Implement encrypt/decrypt functions
3. Update frontend encryption options in `PasteForm.tsx`

### Modifying Paste Storage
1. Implement `PasteStore` trait in `src/lib.rs`
2. Create new adapter (see `src/server/redis.rs` for example)

### Running Tests Before Commit
```bash
cargo fmt --all
cargo clippy --all-targets --all-features
cargo nextest run --workspace --all-features
cd frontend && npm test -- --run && npm run lint
```

---

## Ports Summary

| Service | Port | Purpose |
|---------|------|---------|
| Rocket Backend | 8000 | Main API server |
| Vite Dev Server | 5173 | Frontend development |
| OCaml Crypto | 8001 | Cryptographic verification |

---

## Quick Reference

```bash
# Start development
./scripts/run_both.sh

# Run all tests
cargo nextest run && cd frontend && npm test -- --run

# Build for production
cargo build --release && cd frontend && npm run build

# Docker
docker compose up --build

# Deploy
fly deploy
```

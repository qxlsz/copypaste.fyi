# CLAUDE.md — copypaste.fyi

Lightweight, open-source paste sharing service with client-side encryption, burn-after-reading, post-quantum crypto, steganography, and optional blockchain anchoring.

## Tech Stack

- **Backend**: Rust (2021 edition, 1.82+) with Rocket 0.5.0-rc.3, Tokio async runtime
- **Frontend**: React 19 + Vite 7 + TypeScript, Tailwind CSS, Zustand, TanStack Query, Monaco Editor
- **Crypto Verifier**: OCaml 5.2 service (mirage-crypto) for defense-in-depth dual verification
- **Blockchain**: Hardhat/Solidity (optional paste anchoring)
- **Infra**: Docker multi-stage builds, Docker Compose, Fly.io (process groups)

## Quick Start

```bash
# Backend
cargo build
cargo run --bin copypaste        # http://127.0.0.1:8000

# Frontend dev
cd frontend && npm install && npm run dev   # http://127.0.0.1:5173

# Both together
./scripts/run_both.sh

# Docker
docker compose up --build        # Backend :8000 + Crypto verifier :8001
```

## Testing

```bash
# Rust tests (use nextest, not cargo test)
cargo nextest run --workspace --all-features

# Coverage (must stay >= 75% line coverage)
cargo llvm-cov --workspace --all-features --nextest --fail-under-lines 75

# Lint
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Frontend
cd frontend
npm test -- --run          # Vitest unit tests
npm run lint               # ESLint
npm run test:e2e           # Playwright E2E
```

## Project Layout

```
src/                    Rust backend
  main.rs               Entry point → handlers::launch()
  lib.rs                Core types & traits (PasteStore, StoredPaste, EncryptionAlgorithm)
  server/               14 modules: handlers, crypto, render, attestation, blockchain,
                        redis, webhook, bundles, tor, stego, time, cors, models
  bin/cpaste.rs         CLI client
frontend/               React SPA (pages/, components/, stores/, api/, theme/)
ocaml-crypto-verifier/  Independent crypto verification service (port 8001)
blockchain/             Hardhat/Solidity smart contracts
tests/                  Integration tests (lib_tests.rs, crypto_tests.rs)
scripts/                Dev/build automation (18 scripts)
static/                 Static assets + built frontend (dist/)
```

## Key Architecture Decisions

- **Trait-based storage**: `PasteStore` trait with in-memory default, Redis adapter available
- **Client-side encryption**: Keys never touch the server; encryption/decryption happens in browser
- **Dual crypto verification**: Rust (primary) + OCaml (independent verification)
- **SPA with API**: Frontend is a Vite SPA; backend serves it from `/static/dist` with SPA fallback (but API routes take precedence)

## API Endpoints

- `POST /api/pastes` — Create paste (supports encryption, retention, burn-after-reading)
- `GET /api/pastes/{id}` — Fetch paste as JSON
- `GET /p/{id}` — View paste (HTML)
- `GET /p/{id}/raw` — Raw plaintext
- OCaml service: `POST /verify/encryption`, `POST /verify/signature`, `GET /health`

## CI Pipelines

- **ci.yml**: Rust fmt + clippy + nextest + llvm-cov (75% min), frontend lint + unit tests
- **deploy-fly.yml**: Fly.io deploy on infra changes
- **ocaml-ci.yml**: OCaml build + test
- **release.yml**: CLI binary packaging + GitHub release

## Dev Conventions

- Pre-commit hooks enforce fmt, clippy, nextest — install with `./scripts/setup_git_hooks.sh`
- Install Rust dev tools: `./scripts/install_deps.sh` (fmt, clippy, nextest, llvm-cov)
- Encryption algorithms: AES-256-GCM, ChaCha20-Poly1305, XChaCha20-Poly1305, Kyber hybrid
- See `agents.md` for extended AI agent guide, `docs/encryption.md` for crypto details

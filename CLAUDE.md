# CLAUDE.md — copypaste.fyi

Lightweight, open-source paste sharing service with encryption (including real ML-KEM-768 post-quantum hybrid), burn-after-reading, steganography, attestation gates, time-locks, webhooks, Tor-only pastes, and optional blockchain anchoring.

## Tech Stack

- **Backend**: Rust (2021 edition, 1.82+) with Rocket 0.5.1, Tokio async runtime
- **Frontend**: React 19 + Vite 7 + TypeScript, Tailwind CSS, Zustand, TanStack Query, Monaco Editor, sonner
- **Crypto Verifier**: OCaml 5.2 service (mirage-crypto, Cohttp/Lwt) for defense-in-depth dual verification
- **Blockchain**: Hardhat/Solidity `PasteAnchor` contract (optional, on-demand manifest anchoring via HTTP relayer)
- **Infra**: Docker multi-stage (distroless runtime), Docker Compose, Fly.io (two process groups: `app` + `crypto-verifier`)

## Quick Start

```bash
# Backend (single binary: serve / send / config subcommands)
cargo build
cargo run --bin copypaste -- serve      # http://127.0.0.1:8000

# Frontend dev
cd frontend && npm install && npm run dev   # http://127.0.0.1:5173 (Vite proxies /api → :8000)

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
npm run test:e2e           # Playwright E2E (currently stale — asserts old UI text, no playwright.config)

# Full local CI mirror
./scripts/precommit.sh
```

## Project Layout

```
src/
  lib.rs                Core types: PasteStore trait, MemoryPasteStore, StoredPaste,
                        EncryptionAlgorithm, PersistenceAdapter (+ inline Vault adapter)
  bin/copypaste.rs      The ONLY binary (there is no src/main.rs): clap CLI with
                        `serve`, `send`, `config init` subcommands → handlers::launch()
  server/               15 modules: api_keys, attestation, blockchain, bundles, config,
                        cors, crypto, handlers, models, redis, render, stego, time, tor, webhook
frontend/               React SPA (pages/, components/, stores/, api/, theme/)
ocaml-crypto-verifier/  Independent crypto verification service (port 8001)
blockchain/             Hardhat/Solidity PasteAnchor contract (not gated in CI)
tests/                  Integration tests: lib_tests.rs, crypto_tests.rs, server_tests.rs
scripts/                ~19 dev/build/agent automation scripts (see agents.md)
static/                 Static assets + built frontend (dist/); served with SPA fallback
docs/                   encryption.md, DOCKER_OPTIMIZATION.md, demo.tape, superpowers/ specs
```

## API Endpoints (as registered in `src/server/handlers.rs::build_rocket`)

Pastes:
- `POST /api/pastes` — Create paste (JSON response); `POST /` returns bare path (CLI)
- `GET /api/pastes/{id}?key=&code=&attest=` — Fetch paste as JSON (server decrypts if `key` given;
  the key may also be sent via the `X-Paste-Key` header, which takes precedence over `?key=`)
- `PUT /api/pastes/{id}` / `PATCH /api/pastes/{id}/finalize` — Live-paste update/finalize
  (requires the ownership token from creation as `Authorization: Bearer`)
- `GET /{id}` — HTML view (server-rendered); `GET /raw/{id}` — raw plaintext
  (Note: there are **no** `/p/{id}` backend routes — `/p/:id` is a frontend SPA route only)
- `POST /api/pastes/{id}/anchor` — Blockchain-anchor a paste manifest

Auth & user (Ed25519 challenge–signature; login stores a 24 h in-memory session token):
- `GET /api/auth/challenge`, `POST /api/auth/login`, `POST /api/auth/logout`
- `GET /api/user/paste-count`, `GET /api/user/pastes`, `GET /api/workspaces/{name}/pastes` —
  require `Authorization: Bearer <session token>`; only return the session's own pastes
  (a mismatched `pubkey_hash=` query param is rejected with 403)

Ops & admin:
- `GET /health`, `GET /api/health` (pings OCaml verifier), `GET /api/stats/summary`
- `GET /api/docs` (Scalar UI), `GET /api/openapi.json` (raw OpenAPI 3 document)
- `POST|GET|DELETE /api/admin/keys[/{id}]` — API key CRUD (bearer `COPYPASTE_ADMIN_TOKEN` or SQLite-stored Argon2id keys, per-IP rate limited)

OCaml service: `POST /verify/encryption`, `POST /verify/signature`, `GET /health` (port 8001)

## Key Architecture Decisions

- **Trait-based storage**: `PasteStore` with in-memory default; `PersistenceAdapter` backends selected by `COPYPASTE_PERSISTENCE_BACKEND` = `memory` (default) | `redis` (Upstash REST API, not native protocol) | `vault` (HashiCorp KV v2)
- **Encryption is server-side when a `key` is supplied**: the server derives SHA-256(salt‖key) and encrypts in `spawn_blocking` (`src/server/crypto.rs`). Keys DO transit to the server — do not describe this as zero-knowledge/client-side-only.
- **Algorithms**: AES-256-GCM, ChaCha20-Poly1305, XChaCha20-Poly1305, Kyber hybrid = real **ML-KEM-768** (HKDF-derived deterministic keypair from passphrase; legacy SHA-256-simulation blobs still decryptable)
- **Dual crypto verification**: OCaml re-verifies AES/ChaCha ciphertexts. Advisory by default (log-only); strict mode via `COPYPASTE_REQUIRE_CRYPTO_VERIFICATION=true`. XChaCha20 and Kyber are NOT covered by the OCaml verifier.
- **API keys**: SQLite (rusqlite, `COPYPASTE_SQLITE_PATH`) + Argon2id hashes + failed-attempt rate limiter — the only database in the system; pastes themselves never touch SQLite
- **TOML config** (`src/server/config.rs`): `--config` → `$COPYPASTE_CONFIG` → `./copypaste.toml` → `/etc/copypaste/server.toml`. `bridge_to_env` exports retention (as `COPYPASTE_RETENTION_{DEFAULT,MAX}_MINUTES`), rate limits, and `storage.url` (as `UPSTASH_REDIS_REST_URL` + `REDIS_URL`); paste creation applies the retention default and 400s above the max, and `rate_limit::PasteRateLimiter` enforces per-IP create/read limits (disabled when the env knobs are unset). Caveat: `storage.path` and `auth.token` are bridged but nothing consumes `COPYPASTE_AUTH_TOKEN` yet.
- **SPA with API**: backend serves `static/` with a rank-100 SPA fallback; API routes take precedence

## Environment Variables (main ones)

- Storage: `COPYPASTE_PERSISTENCE_BACKEND`; Redis: `UPSTASH_REDIS_REST_URL/_TOKEN`, `COPYPASTE_REDIS_KEY_PREFIX`; Vault: `COPYPASTE_VAULT_ADDR/_TOKEN/_MOUNT/_NAMESPACE/_PREFIX`
- Crypto: `CRYPTO_VERIFIER_URL` (default `http://localhost:8001`), `COPYPASTE_REQUIRE_CRYPTO_VERIFICATION`
- Limits: `COPYPASTE_MAX_PASTE_SIZE` (default 10 MiB)
- Admin: `COPYPASTE_ADMIN_TOKEN`; Tor: `COPYPASTE_ONION_HOST`, `COPYPASTE_TOR_SUPPRESS_LOGS`
- Anchoring: `ANCHOR_RELAY_ENDPOINT`, `ANCHOR_RELAY_API_KEY`

## Known Half-Built / Gotchas (verify before relying on)

- Auth challenges are not stored server-side: `POST /api/auth/login` verifies the signature over whatever challenge string the client supplies, so a captured (challenge, signature) pair can be replayed to mint a new session
- Login sessions live in an in-memory `SessionStore` — lost on restart and not shared across instances/process groups
- Webhook SSRF validation blocks IP literals and `localhost`/`.internal`/`.local` names but does not resolve DNS, so a public hostname that resolves to an internal IP (DNS rebinding) is not caught; redirects are disabled as a mitigation
- `auth.token` from the TOML config is bridged to `COPYPASTE_AUTH_TOKEN`, but no middleware consumes it yet
- `WorkspacePasteQuery` in models.rs is still unused (the workspace listing route uses a path param instead)

## CI Pipelines (`.github/workflows/`)

- **ci.yml**: fmt, clippy (-D warnings), nextest, llvm-cov (75% gate), frontend lint + vitest
- **ocaml-ci.yml**: dune build + test + docker smoke test (on `ocaml-crypto-verifier/**`)
- **deploy-fly.yml**: Fly.io rolling deploy (`Dockerfile.backend`, both process groups)
- **docker-publish.yml**: multi-arch ghcr.io image (`edge` + `latest`)
- **release.yml**: 4-target binary matrix + crates.io publish + SLSA provenance/SBOM
- **auto-research.yml**: daily competitive-intel cron → files a GitHub issue

## Dev Conventions

- Pre-commit hooks enforce fmt, clippy, nextest — install with `./scripts/setup_git_hooks.sh`
- Install Rust dev tools: `./scripts/install_deps.sh`
- Development is largely issue-driven via `scripts/agent.sh` (autonomous issue→PR pipeline)
- See `agents.md` for the extended module map, `docs/encryption.md` for crypto details

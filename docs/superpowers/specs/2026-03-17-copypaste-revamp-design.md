# copypaste.fyi Revamp — Design Spec

**Date:** 2026-03-17
**Status:** Draft
**Goal:** Transform copypaste.fyi from a crypto-demo paste tool into a star-worthy, self-hostable, CLI-first paste sharing utility. Target: 1000+ GitHub stars.

---

## 1. Core Identity

**Tagline:** Fast, encrypted paste sharing from your terminal.

**What it is:** A single Rust binary that does two things:
1. **Client** — pipe anything from your terminal to a shareable link
2. **Server** — self-host your own paste service with zero config

**What it is NOT:** A crypto research demo, a blockchain tool, or a steganography showcase.

---

## 2. CLI Architecture

Single binary `copypaste` with subcommands (clap).

### Commands

```
copypaste send [OPTIONS] [CONTENT]     # create paste
copypaste get <ID> [OPTIONS]           # retrieve paste
copypaste serve [OPTIONS]              # start server
copypaste config [OPTIONS]             # manage config
copypaste list [OPTIONS]               # list recent pastes (auth'd)
copypaste delete <ID>                  # delete paste (auth'd)
copypaste version                      # version info
```

### Key UX Flows

```bash
# Positional content
copypaste send "hello world"

# Stdin pipe (primary use case)
cat crash.log | copypaste send
kubectl logs pod-xyz | copypaste send --burn
cargo test 2>&1 | copypaste send --expire 1h

# Encrypted (AES-256-GCM default)
copypaste send --encrypt "secret stuff"
# → https://copypaste.fyi/p/x7k9m2#key=base64encodedkey
# Key in URL fragment — never hits the server

# Self-host target
copypaste send --host http://localhost:8000 "internal stuff"

# Retrieve
copypaste get x7k9m2
copypaste get x7k9m2 --raw
copypaste get x7k9m2 | pbcopy

# JSON output for scripting
copypaste send --json "data" | jq .url
```

### Client Config (`~/.config/copypaste/config.toml`)

```toml
host = "https://copypaste.fyi"
default_expire = "24h"
default_encrypt = false
# auth_token = "..."
```

---

## 3. Server Architecture

### What Stays

- Rocket web server with paste CRUD API
- AES-256-GCM + ChaCha20-Poly1305 encryption
- Burn-after-reading
- Configurable retention/expiry
- Health endpoint
- SPA frontend serving with API route priority

### What Gets Dropped

- OCaml crypto verifier (complexity for self-hosters)
- Steganography (niche, bloats binary)
- Blockchain anchoring (novelty, not utility)
- Post-quantum Kyber (add back later as feature flag)
- Tor onion config (users can set up externally)
- TOTP/attestation (half-built, revisit later)
- Webhook system (revisit later)
- Bundles (revisit later)
- Time-lock (revisit later)
- Vault persistence adapter (too niche)

### What Gets Added

- **SQLite as default persistence** (replaces in-memory for self-hosters)
- **Bearer token auth** for write operations
- **Config file support** (`/etc/copypaste/server.toml` or env var)
- **Structured JSON logging**
- **Prometheus metrics** at `/metrics`
- **Graceful shutdown** with data flush
- **Rate limiting** (configurable per-IP)
- **Background expiry reaper** (every 60s)

### Persistence Tier

```bash
copypaste serve                              # SQLite (default), ./copypaste.db
copypaste serve --store memory               # in-memory (ephemeral)
copypaste serve --store sqlite://path/to.db  # custom SQLite path
copypaste serve --store redis://host:6379    # Redis
```

### Server Config (`/etc/copypaste/server.toml`)

```toml
[server]
address = "0.0.0.0"
port = 8000
max_paste_size = "10mb"

[storage]
backend = "sqlite"
path = "/var/lib/copypaste/data.db"

[auth]
token = "your-secret-token"   # optional; if set, requires Bearer token for writes

[retention]
default = "24h"
max = "30d"

[rate_limit]
creates_per_minute = 60
reads_per_minute = 300

[logging]
format = "json"               # or "pretty"
level = "info"
```

Env vars override config: `COPYPASTE_AUTH_TOKEN`, `COPYPASTE_PORT`, etc.

---

## 4. Distribution & Packaging

### Homebrew

Homebrew tap with pre-built binaries (no source compilation).

```bash
brew tap USER/copypaste
brew install copypaste
```

Includes: binary, bash/zsh/fish completions, man page.

Formula uses pre-built release artifacts per platform (darwin-arm64, darwin-x64, linux-amd64).

### Linux

```bash
# One-liner install script
curl -fsSL https://copypaste.fyi/install.sh | sh

# Manual download
wget https://github.com/USER/copypaste.fyi/releases/download/v0.2.0/copypaste-linux-amd64.tar.gz
tar xzf copypaste-linux-amd64.tar.gz
sudo mv copypaste /usr/local/bin/
```

### GitHub Release Artifacts

Per release tag:
- `copypaste-darwin-arm64.tar.gz` (macOS Apple Silicon)
- `copypaste-darwin-x64.tar.gz` (macOS Intel)
- `copypaste-linux-amd64.tar.gz` (Linux x86_64)
- `copypaste-linux-arm64.tar.gz` (Linux ARM64)
- `checksums.txt` (SHA-256)

Cross-compiled via GitHub Actions using `cross` or `cargo-zigbuild`.

### Docker

```bash
docker pull ghcr.io/USER/copypaste:latest
docker run -p 8000:8000 -v ./data:/data ghcr.io/USER/copypaste:latest
```

Multi-arch image (linux/amd64 + linux/arm64). Single-stage, no OCaml, no Node.

```yaml
# docker-compose.yml
services:
  copypaste:
    image: ghcr.io/USER/copypaste:latest
    ports: ["8000:8000"]
    volumes: ["./data:/data"]
    environment:
      COPYPASTE_AUTH_TOKEN: "changeme"
```

### Release CI

GitHub Actions workflow triggered on `v*` tags:
1. Cross-compile for 4 targets (macOS arm64/x64, Linux amd64/arm64)
2. Generate shell completions + man page
3. Create GitHub release with artifacts + checksums
4. Push multi-arch Docker image
5. Update Homebrew tap formula

---

## 5. Web UI Revamp

### Design Philosophy

Kill the "crypto research paper" vibe. Dark-first, minimal, fast, with micro-interactions that make developers go "damn."

Inspiration: Vercel dashboard, Linear, Raycast.

### Pages (3 total)

**Page 1: Composer (`/`)**
- Full-viewport Monaco editor (80% of screen)
- Inline toolbar below: language selector, expiry dropdown, encrypt toggle, burn toggle, send button
- After send: editor slides up, shareable link + CLI command appear
- Cmd+Enter to submit
- Auto-detect language from content

**Page 2: Viewer (`/p/:id`)**
- Read-only with syntax highlighting (Shiki for lighter weight)
- Metadata bar: language, expiry countdown, encryption status
- Action bar: copy content, view raw, create new
- CLI hint at bottom: `copypaste get x7k9m2`
- Decrypt flow: single key input → shimmer animation → content reveals
- Burn warning: red banner if burn-after-reading

**Page 3: Dashboard (`/dashboard`)** (auth'd)
- Clean table: ID, language, encryption badge, time remaining, view count, delete
- Search/filter by language, encrypted, active/expired
- Bulk delete

### What Gets Removed from UI

- About page (move to docs/README)
- Stats page (replace with minimal footer status)
- Privacy Journey indicator
- Dashboard account info tab (raw crypto keys)
- Steganography UI
- Blockchain UI
- Command palette
- D3 charts, Mermaid diagrams

### Visual Design

**Dark-first palette:**
- Background: `#0a0a0a`
- Surface: `#141414`
- Border: `#262626`
- Text primary: `#fafafa`, secondary: `#a1a1a1`
- Brand: `#3b82f6` (blue)
- Success: `#22c55e` (green)
- Burn: `#ef4444` (red)
- Encrypt: `#a855f7` (purple)

**Typography:** Inter (UI), JetBrains Mono (code), 13px base.

**Micro-interactions:**
- Copy button: click → checkmark animation → "Copied!" (2s)
- Send: button pulses → content fades → link slides in
- Encrypt toggle: lock icon animates shut
- Burn toggle: flame flickers on hover
- Decrypt: shimmer across encrypted blob → content reveals
- Theme toggle: smooth 150ms color transition

### Tech Stack Changes

- **Keep:** React 19, Vite 7, TypeScript, Tailwind CSS
- **Drop:** D3, Mermaid, TanStack Query, Zustand (React context sufficient)
- **Add:** Framer Motion (animations), Shiki (syntax highlighting for viewer)
- **Consider:** CodeMirror 6 instead of Monaco (smaller bundle, better mobile)

---

## 6. Security Hardening

### Server

- **Input validation:** max 10MB paste, nanoid 12-char IDs, Content-Type enforcement
- **Rate limiting:** 60 creates/min, 300 reads/min per IP (configurable)
- **Encryption:** AES-256-GCM + ChaCha20-Poly1305, Argon2id key derivation (replaces SHA-256)
- **Auth:** Bearer token for writes, HMAC-SHA256 signed tokens with expiry, constant-time comparison
- **Headers:** strict CSP, X-Content-Type-Options, X-Frame-Options: DENY, Referrer-Policy: no-referrer, HSTS 1yr
- **Storage:** SQLite WAL mode, expired paste reaper, burn-after-reading deletes in same transaction
- **Logging:** structured JSON, request IDs, hashed IPs, never log content
- **Dependencies:** `cargo audit` + `cargo deny` in CI

### Client

- Key never leaves client — URL fragment only
- No telemetry, no analytics, no phone-home
- Config file permissions: 600

---

## 7. Testing Strategy

### Backend (Rust) — 85%+ coverage

```
tests/
  unit/
    crypto_test.rs          # encrypt/decrypt round-trips
    persistence_test.rs     # SQLite + Redis + memory
    auth_test.rs            # token generation, validation, expiry
    retention_test.rs       # expiry, burn-after-reading
    models_test.rs          # serialization, validation
  integration/
    api_test.rs             # full HTTP CRUD flows
    auth_flow_test.rs       # login, authenticated operations
    rate_limit_test.rs      # rate limiter behavior
    encryption_e2e_test.rs  # encrypted paste lifecycle
  fuzz/
    paste_input.rs          # fuzz API inputs
```

Property-based tests with `proptest` for crypto round-trips and ID uniqueness.

### Frontend — 80%+ coverage

- Component tests: Vitest + Testing Library
- E2E: Playwright (create → view → copy → decrypt)
- Visual regression: Playwright screenshots (dark/light, mobile/desktop)

### CLI

- Unit: argument parsing, config loading
- Integration: real server round-trips
- Snapshot: help output, error messages

### CI Pipeline

```yaml
- cargo fmt --check
- cargo clippy -- -D warnings
- cargo nextest run --workspace (85% coverage gate)
- cargo audit
- cargo deny check
- cd frontend && npm test -- --run (80% coverage gate)
- cd frontend && npx playwright test
- cargo build --release
```

---

## 8. Roadmap

### Phase 1: Foundation (the MVP that gets stars)

- Rebrand `cpaste` → `copypaste` single binary with subcommands
- CLI: `send`, `get`, `serve`, `config`, `version`
- Server: SQLite default, memory optional, Redis optional
- Drop: OCaml verifier, stego, blockchain, Kyber, webhooks, bundles, time-lock, attestation
- Security: rate limiting, security headers, Argon2id, cargo audit
- Frontend: 3-page dark-first redesign (composer, viewer, paste list)
- Tests: 85% backend, 80% frontend
- Distribution: Homebrew tap, Linux binary, Docker, install.sh
- README: animated terminal GIF, comparison table, one-liner install/usage/self-host

### Phase 2: Polish (makes people tell friends)

- Auto-detect language from content
- Shell completions (bash, zsh, fish)
- Man page generation
- `copypaste list` with local history
- Framer Motion micro-interactions
- Mobile-optimized viewer (CodeMirror 6 or Shiki)
- Homebrew core submission
- `.deb` / `.rpm` packages

### Phase 3: Power Features (builds community)

- Team features: workspaces, API keys, simple RBAC
- Webhook notifications (Slack, Discord, generic)
- Post-quantum encryption (Kyber) as opt-in
- S3 persistence backend
- Prometheus metrics endpoint
- `copypaste watch` — live log sharing (tail + auto-update)
- Plugin system for custom backends

### Phase 4: Ecosystem

- GitHub Action
- VS Code extension
- Raycast extension
- Neovim plugin
- Federation between instances

---

## 9. Success Criteria

- `brew install copypaste && echo "hello" | copypaste send` works in under 30 seconds
- `copypaste serve` starts a fully functional server with zero config
- README has animated terminal GIF that sells the tool in 5 seconds
- Frontend loads in under 1 second, looks clean enough to screenshot
- All encryption is client-side, server never sees plaintext
- 85%+ backend test coverage, 80%+ frontend
- Zero `cargo audit` advisories
- Ships on Hacker News with "Show HN" post

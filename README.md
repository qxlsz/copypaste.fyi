<div align="center">

# copypaste.fyi

Type. Get link. Share.

[copypaste.fyi](https://www.copypaste.fyi) · [First principles](docs/first-principles.md) · [Self-host](docs/self-host.md) · [Packaging](docs/packaging.md)

[![CI](https://github.com/qxlsz/copypaste.fyi/actions/workflows/ci.yml/badge.svg)](https://github.com/qxlsz/copypaste.fyi/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

```text
$ copypaste send "notes from the incident"
https://www.copypaste.fyi/p/AbCdEf12GhJkLmNpQrStUvWxYz0123456789abcdef
```

On the site: type, tap **Get link**. The URL is on the clipboard. No account. No listing.

## Server and client

One binary. The server stores pastes. The client creates them.

```bash
# server, this machine
ROCKET_ADDRESS=127.0.0.1 COPYPASTE_FORCE_MEMORY=true copypaste serve

# client, same machine or another
export COPYPASTE_HOST=http://127.0.0.1:8000
copypaste send "from the client"
copypaste clip
```

| Install | Server | Client |
|---|---|---|
| Homebrew | `brew install qxlsz/copypaste/copypaste` then `brew services start copypaste` | `copypaste send --host http://127.0.0.1:8000 "..."` |
| Debian | `./scripts/build-deb.sh target/release/copypaste 0.2.0 dist` then `apt install ./dist/copypaste_*.deb` | same `send --host` |
| Docker | `docker compose up --build` | `send --host http://127.0.0.1:8000` |
| Agent VM | `./scripts/agent-setup.sh --serve` | `send --host http://127.0.0.1:8000` |
| Public site only | you do not run a server | `copypaste send --host https://www.copypaste.fyi "..."` |

Cookbook: [docs/self-host.md](docs/self-host.md). Deb and brew test: [docs/packaging.md](docs/packaging.md). Why it works this way: [docs/first-principles.md](docs/first-principles.md).

Lock a private server with `COPYPASTE_REQUIRE_WRITE_AUTH=true` and `COPYPASTE_AUTH_TOKEN`. Clients send `X-CopyPaste-Write-Token`. Never put the token on argv.

Dev UI + API from a clone: `./scripts/install_deps.sh` then `ROCKET_ADDRESS=127.0.0.1 ./scripts/run_both.sh`.

**Mac Quick Action** (right-click selected text):

```bash
chmod +x contrib/macos/*.sh
./contrib/macos/install-quick-action.sh
```

Then Services → Send to copypaste. `COPYPASTE_HOST=http://127.0.0.1:8000` points it at your instance.

**iTerm2, Ghostty, cmux:** select text, then send that copy to your server so the clipboard holds the share URL.

```bash
./contrib/terminals/install.sh
copypaste clip --host http://127.0.0.1:8000
```

iTerm2: Pointer → Right button → Run Command in Background → `copypaste clip --host http://127.0.0.1:8000`.
Ghostty and cmux share `~/.config/ghostty` (`copy_on_select`, then `copypaste clip`). Notes: [docs/terminals.md](docs/terminals.md).

## What it does

- 43-character IDs, optional 10-character alphanumeric short alias, 1 MiB cap
- Optional **server-side** AES-256-GCM / ChaCha20-Poly1305 (OCaml verifier on a private VM)
- JSON API, `/p/{id}` share pages, `/raw/{id}`, secret-file-aware CLI
- Optional Upstash Redis REST, Tor ingress, admin quarantine, content-safe anchoring

It does **not** do browser-only E2E encryption, public paste search, or exactly-once burn across multiple app instances.

## Architecture

Browser and CLI talk HTTPS to one Rocket process. Pastes live in that process unless you point it at Redis. Optional AES/ChaCha checks can call the OCaml verifier. Run one `app` instance. Sessions, stats, and burn consume are process-local. `COPYPASTE_FORCE_MEMORY=true` beats a leftover Redis secret.

## Security

There is **no paste listing**. Canonical IDs are 43 random characters. Missing, burned, and expired reads all return the same `404 paste_not_found`. Create/read are process-local rate limited; put an edge quota in front of a public box.

Encryption happens in the Rust service (AES-256-GCM / ChaCha20-Poly1305). TLS to the edge is required; plaintext and the key can still exist in the app (and, for AES/ChaCha, in the OCaml verifier) while a paste is written. Keys are not stored on purpose. This is **not** zero-knowledge.

Burn-after-reading is best-effort. Link previews can consume a burn paste.

Hiding bytes in an image is not a security boundary. Server steganography stays off.

Read [SECURITY.md](SECURITY.md) and the [abuse runbook](docs/abuse-response.md) before putting an instance on the internet.

## API

| Method | Route | |
|---|---|---|
| `POST` | `/api/pastes` | Create. Public site: no token. Closed: write token. |
| `GET` | `/api/pastes/{id}` | JSON. Encrypted: `X-Paste-Key` |
| `GET` | `/p/{id}` | Share page |
| `GET` | `/raw/{id}` | Raw body |
| `GET` | `/api/stats/summary` | This-instance counts |
| `GET` | `/.well-known/copypaste.json` | Agent discovery (tokens, encrypt, 404 sameness) |

```bash
curl -sS -X POST https://www.copypaste.fyi/api/pastes \
  -H 'content-type: application/json' \
  -d '{"content":"hello from curl","format":"plain_text"}'
```

`401` missing key · `403` bad key · `404` missing, burned, or expired · `423` JSON time-lock · `410` raw after the time-lock window · `503` storage down.

## CLI

```bash
copypaste send "notes"
echo "log" | copypaste send --stdin --host https://www.copypaste.fyi
copypaste send --clipboard --host https://www.copypaste.fyi
copypaste clip --host http://127.0.0.1:8000
export COPYPASTE_HOST=http://127.0.0.1:8000
copypaste clip
copypaste send --json "plain receipt for an agent"
copypaste send --agent "only the other agent can read this"
copypaste send --auth-token-file ./write-token "closed instance"
copypaste send --encryption-mode aes256_gcm --encryption-key-file ./paste-key "secret"
copypaste healthcheck --host http://127.0.0.1:8000
```

`send`, `clip`, and `healthcheck` read `COPYPASTE_HOST` when `--host` is omitted. `--host` still wins.

The CLI never takes a token or encryption key as a flag value. Remote hosts must be HTTPS. Human mode prints the key-free share URL. `--json` / `--agent` print a receipt with the tokens the other side needs.

## Agents

Another model only needs the receipt. `GET /.well-known/copypaste.json` is the map.

```bash
# Agent A
copypaste send --agent --host https://www.copypaste.fyi "task for B"
# → {"copypaste":1,"url":"https://…/p/…","key":"…","headers":{"X-Paste-Key":"…"}}

# Agent B
curl -sS -H "X-Paste-Key: $KEY" "$GET"
```

On the site: Get link → **Agent**, then **Open with Grok / Codex / ChatGPT / Claude**. Keys never go in those URLs. **Add to Grok** copies a Grok Bot skill and opens Grok.

- [llms.txt](https://www.copypaste.fyi/llms.txt)
- [Grok Bot](https://www.copypaste.fyi/grok-bot.md)
- [Discovery](https://www.copypaste.fyi/.well-known/copypaste.json)

Encrypted pastes stay ciphertext for anyone who only has the URL. This is still **server-side** encryption, not E2E.

Verify a local install:

```bash
./scripts/verify-install.sh
```

## Configure

| Variable | Public site | Purpose |
|---|---|---|
| `COPYPASTE_REQUIRE_WRITE_AUTH` | `false` | `true` locks `POST /api/pastes` |
| `COPYPASTE_AUTH_TOKEN` | unset | Write admission, 43–128 base64url in production |
| `COPYPASTE_FORCE_MEMORY` | `true` | Ignore Redis secrets; stay in RAM |
| `COPYPASTE_PERSISTENCE_BACKEND` | `memory` | `memory` or `redis` |
| `UPSTASH_REDIS_REST_URL` / `_TOKEN` | unset | Required when backend is `redis` |
| `COPYPASTE_MAX_PASTE_SIZE` | `1048576` | Cap, max 1 MiB |
| `COPYPASTE_ALLOWED_ORIGINS` | copypaste.fyi | Exact CORS list |
| `CRYPTO_VERIFIER_URL` | Fly private DNS | OCaml verifier |
| `COPYPASTE_REQUIRE_CRYPTO_VERIFICATION` | `true` | Fail create if verifier fails |

Full list and Tor/SQLite/admin notes: [CLAUDE.md](CLAUDE.md). Bundles, webhooks, attestations, and steganography stay off — turning their old flags on refuses to start.

## Deploy

Frontend: Vercel (`frontend/`, `vercel.json`). Backend: Fly (`fly.toml`, `Dockerfile.backend`). Image: `ghcr.io/qxlsz/copypaste.fyi`.

```bash
fly deploy
fly logs
```

Keep `app` at one machine. The verifier is a separate process group with no public port.

## Test

Same jobs as [CI](https://github.com/qxlsz/copypaste.fyi/actions/workflows/ci.yml):

```bash
python3 scripts/lint-workflows.py
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo llvm-cov nextest --workspace --all-features --fail-under-lines 75
cd frontend && npm ci && npm test -- --run && npm run lint && npm run build
```

Shortcut: `./scripts/precommit.sh`. Agents: [AGENTS.md](AGENTS.md). Humans: [CONTRIBUTING.md](CONTRIBUTING.md).

## Layout

```text
src/bin/copypaste.rs     serve / send / clip / healthcheck / config
src/server/              Rocket, crypto, Redis, guards
frontend/                React 19 + Vite composer
ocaml-crypto-verifier/   Independent AES/ChaCha checks
.github/workflows/       CI, Fly, GHCR, OCaml, release
```

## License

[Apache-2.0](LICENSE).

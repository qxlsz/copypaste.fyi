# AGENTS.md

Instructions for Grok, Claude, Codex, and any other agent working in **qxlsz/copypaste.fyi**.

This file is the product + CI contract. [CLAUDE.md](CLAUDE.md) has extra route/auth detail. [CONTRIBUTING.md](CONTRIBUTING.md) is for humans.

## Product

copypaste.fyi is a pastebin. The whole product is:

1. Type (or pipe) text
2. **Get link**
3. Share `/p/{id}`

If a visitor cannot find Get link, the change is wrong. Phones (~390×844) must keep Get link on-screen, in the thumb zone, and above the software keyboard (`visualViewport` → `--keyboard-inset`). Format / expiry / burn / encrypt may sit behind a summary row on small screens. Desktop may keep them inline.

Do **not** bring back: encryption popups, a floating privacy chip, command-palette login, always-on write-token fields on the public composer, dashboard chrome on About/Stats, or a 100% min-height editor that pushes the dock below the fold.

Public [copypaste.fyi](https://www.copypaste.fyi) is anonymous. `fly.toml` has `COPYPASTE_REQUIRE_WRITE_AUTH=false` and `COPYPASTE_FORCE_MEMORY=true` on purpose. Do not “fix” that back to a locked 401 or Redis 503. Self-hosters lock writes with `COPYPASTE_REQUIRE_WRITE_AUTH=true`.

Encryption is **server-side**. Never call it E2E, client-side, or zero-knowledge. Keys may exist in Rocket (and the OCaml verifier) during create/read.

## Stack

Rust 1.88 / Rocket 0.5 · React 19 / Vite 7 / TypeScript · OCaml 5.2 verifier · Fly `app` + `crypto-verifier` on separate VMs · Vercel for `frontend/` · one `copypaste` binary (`serve`, `send`, `healthcheck`, `config init`).

## Grok workflow (every change)

1. Read this file. Match the product above.
2. Change the smallest surface that fixes the request.
3. Run the CI jobs that cover what you touched (table below). Workflow YAML: `python3 scripts/lint-workflows.py`. Duplicate top-level keys (`permissions:` twice) make GitHub fail as a **0-job** run named `.github/workflows/<file>.yml` on every push.
4. Push a branch, open a PR.
5. **Wait for every Actions run on that SHA.** `ci.yml` green is not enough. Invalid workflows, Auto Research, Docker publish, Fly deploy all count. A cancelled-by-newer Docker publish is fine. `npm audit --audit-level=high` is part of Frontend; a new GHSA on a lockfile is a red CI, not a fluke.
6. If anything is red: fix on the same branch, push, wait again. Do not merge red. Do not leave `main` red.
7. Merge only when the PR checks are green. After merge, confirm `CI` on `main` succeeded and Auto Research did **not** fail.

Auto Research is **manual** (`workflow_dispatch` only). It writes a competitor briefing issue. It does not ship code. Do not re-enable a daily cron; it flooded the tracker.

## CI jobs

`.github/workflows/ci.yml` — Rust **1.88.0**, Node **22**:

| Job | Command |
|---|---|
| Workflow lint | `python3 scripts/lint-workflows.py` |
| Rust fmt | `cargo fmt --all -- --check` |
| Rust clippy + tests | `cargo audit` · `cargo clippy --all-targets --all-features -- -D warnings` · `cargo nextest run --workspace --all-features` |
| Coverage | `cargo llvm-cov nextest --workspace --all-features --fail-under-lines 75` |
| Frontend | `cd frontend && npm ci && npm audit --audit-level=high && npm run lint && npm test -- --run && npm run build && cmp vercel.json frontend/vercel.json` |

OCaml tree: `.github/workflows/ocaml-ci.yml`. Local bundle: `./scripts/precommit.sh`. Catalog: [`.github/workflows/README.md`](.github/workflows/README.md).

## Layout (what actually exists)

```text
src/bin/copypaste.rs           serve / send / healthcheck / config
src/lib.rs                     store, cache, persistence boundary
src/server/handlers.rs         routes, guards, moderation
src/server/crypto.rs           server-side crypto + verifier client
src/server/redis.rs            Upstash REST; FORCE_MEMORY bypasses it
frontend/src/pages/PasteForm.tsx   composer — Get link lives here
frontend/src/pages/PasteView.tsx   share page
frontend/src/hooks/useKeyboardInset.ts
ocaml-crypto-verifier/         AES-GCM + ChaCha20-Poly1305 checks
fly.toml                       public site policy
.github/workflows/             CI, Fly, GHCR, OCaml, release, auto-research
```

Disabled in the hardened build (requests rejected; enabling flags abort startup): webhooks, attestations, steganography, bundles. Do not “restore” them.

## Verify by using it

Not just unit tests:

- `POST /api/pastes` → 200 JSON with `shareableUrl` (no 401/503 on the public policy)
- `GET /api/stats/summary` → JSON
- Phone viewport: Get link bounding box fully inside 390×844
- After create: Copy works; Share uses `navigator.share` when present
- `GET /.well-known/copypaste.json` → `copypaste: 1`
- `copypaste send --agent` prints JSON tokens; GET without `X-Paste-Key` is 401
- Open-with Grok/Codex/ChatGPT/Claude must strip `#key=`; never send secrets to a third-party chat URL

## Agents

AI-to-AI is the same pastebin with a machine receipt. Tokens live in JSON / headers (`X-Paste-Key`, `X-CopyPaste-Write-Token`), never in argv. `--agent` encrypts (AES-256-GCM) so a human with only the URL cannot read the body. Do not call that E2E.

## Don’t

- Invent a second UI language or a new pastebin product around the composer
- Put secrets in CLI flags, URLs, or commit messages
- Add `include_str!` of a file that `Dockerfile` / `.dockerignore` drop. GHCR copies `static/`; `*.md` in `.dockerignore` must un-ignore `static/grok-bot.md`
- Scale `app` past one Fly machine
- Describe `/health` as a Redis/verifier probe
- Close GitHub issues as “fixed” without the code on `main`

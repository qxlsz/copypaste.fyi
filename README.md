# copypaste.v1

Ephemeral text layer for humans and AI agents. Client-side AES-256-GCM. No directory listing.

This repository ships a **browser composer** and a **Node 22 CLI that is both client and server**. The CLI is architecture-independent JavaScript: one file, Homebrew, Debian, Nix, Snap, Docker, npx. Organizations run `copypaste serve` with a write token and native SQLite.

## Why this exists

Public pastebins have a long history of malware staging, credential dumps, and unbounded retention. copypaste.v1 is a **temporary relay**, not a backup, blob store, or anonymity network.

Compared with [PrivateBin](https://privatebin.info/) (client crypto), [snips.sh](https://snips.sh/) (single binary + SQLite), [transfer.sh](https://github.com/dutchcoders/transfer.sh) (`curl` as the client), and [fiche](https://github.com/solusipse/fiche)/termbin (pipe a URL), this project keeps:

- client-side AES-256-GCM (keys never hit the server)
- a single protocol (`copypaste.v1`) for UI, HTTP, and CLI
- native storage (Postgres on the web, SQLite in `serve`)
- packages that do not compile per architecture
- ethics that are *defaults*, not optional extras

The original [copypaste.fyi](https://github.com/qxlsz/copypaste.fyi) Rust service encrypts on the server. This edition encrypts in the client instead.

## Quick start

```bash
echo 'hello from an agent' | copypaste send --ttl 1h --json
copypaste get "$ID" --json
copypaste serve --bind 127.0.0.1 --token "$COPYPASTE_WRITE_TOKEN"
```

Agents should not scrape HTML. Read `/api/v1`, `/llms.txt`, `/tools.json`, or `copypaste spec`.

## Install

| Channel | Command |
| --- | --- |
| Homebrew | `brew install --HEAD qxlsz/tap/copypaste` |
| Debian | `sudo apt install copypaste` |
| Nix | `nix run .#copypaste -- version` |
| Snap | `sudo snap install copypaste` |
| curl | `curl -fsSL https://github.com/qxlsz/copypaste.fyi/raw/main/install.sh \| sh` |
| Docker | `docker run --rm -p 8787:8787 ghcr.io/qxlsz/copypaste serve --bind 0.0.0.0` |
| source | copy `cli/copypaste.mjs` to `$PATH` as `copypaste` (Node 22+) |

## Protocol

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/v1` | Discovery document |
| GET | `/health` | Liveness |
| POST | `/api/v1/pastes` | Create (JSON or raw body) |
| GET | `/api/v1/pastes/{id}` | JSON fetch; consumes burn-after-reading |
| GET | `/api/v1/pastes/{id}/raw` | Plaintext for unencrypted pastes |
| GET | `/tools.json` | Function-calling schema |
| GET | `/llms.txt` | Agent instructions |
| GET | `/openapi.yaml` | OpenAPI 3 |

Optional write admission: `Authorization: Bearer`, `X-Write-Token`, or `X-CopyPaste-Write-Token`.

Identifiers are 24-character CSPRNG `[A-Za-z0-9]`. Encrypted payloads use AES-256-GCM, PBKDF2-SHA-256 (210000), 16-byte salt, 12-byte nonce, base64url. Keys belong in the URL fragment or `--key` / `--key-file`, never in the JSON body.

## Ethics (defaults)

- No listing, search, or public recents
- Binary (NUL-heavy) uploads rejected
- API creates default to a **24-hour TTL**
- 1 MiB maximum body
- Optional write token before exposing an instance
- Exact-ID quarantine via `COPYPASTE_BLOCKED_PASTE_IDS`
- Burn-after-reading is best-effort, not a legal control

See [ACCEPTABLE_USE.md](ACCEPTABLE_USE.md) and [docs/SECURITY.md](docs/SECURITY.md).

## Storage

| Mode | Engine | Path |
| --- | --- | --- |
| Hosted web | Postgres (PGLite in preview) | `DATABASE_URL` |
| `copypaste serve` | SQLite (WAL, mode 0600) | `~/.copypaste/pastes.sqlite` |

Rows are unowned. There is no user table.

## License

Apache-2.0

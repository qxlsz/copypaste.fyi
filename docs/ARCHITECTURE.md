# Architecture

copypaste.v1 is a **temporary, unlisted text layer** for humans, programs, and
AI agents. It is a pastebin, not a backup, blob store, or anonymity network.

## Comparables (and what we took)

| System | Lesson | What we did not copy |
| --- | --- | --- |
| [PrivateBin](https://privatebin.info/) | Client-side encryption; server holds ciphertext | PHP stack, discussion threads |
| [snips.sh](https://snips.sh/) | Single binary, SQLite, self-host, agent-friendly I/O | SSH-only UX |
| [transfer.sh](https://github.com/dutchcoders/transfer.sh) | `curl` as the client; packages and containers | File/blob hosting, S3 backends |
| [fiche](https://github.com/solusipse/fiche) / termbin | Pipe to a port, get a URL | Unencrypted by default, no TTL ethics |
| [0x0.st](https://0x0.st/) | Short-lived hosting, size caps | Public file hosting |
| Original [copypaste.fyi](https://github.com/qxlsz/copypaste.fyi) | 24-char ids, burn, write admission, 1 MiB | Server-side encryption, Redis, API keys as product |

Public pastebins have a long history of malware staging, credential dumps, and
unbounded retention. The ethics in this protocol — no listing, no binaries,
default TTL, optional write token, exact-ID quarantine — exist because of that.

## Two runtimes, one protocol

1. **Hosted web** (this app): TanStack Start, PGLite in preview / Postgres in
   production. Browser composer + REST (`/api/v1`) + raw routes.
2. **CLI** (`cli/copypaste.mjs`): Node 22+ client *and* server. `copypaste serve`
   is an air-gapped org node with native SQLite (`node:sqlite`, WAL, mode 0600)
   under `~/.copypaste` or `COPYPASTE_DATA_DIR`. The same process serves
   `/api/v1`, `/tools.json`, `/llms.txt`, and `/openapi.yaml` so agents do not
   need the web UI.

Agents should not scrape HTML. They should:

- `GET /api/v1` or `GET /llms.txt` or `GET /tools.json`
- `POST /api/v1/pastes`
- or `echo body | copypaste send --json --ttl 1h`

## Packaging

The CLI is architecture-independent JavaScript with a `node` shebang. Homebrew
depends on `node` (head formula until a tagged tarball exists). Debian
`Architecture: all` depends on `nodejs (>= 22)`. Nix and Snap wrap the same
file. Docker is `node:22-bookworm-slim`. There is no per-arch compile step.

## Storage

| Mode | Engine | Path |
| --- | --- | --- |
| Preview | PGLite (Postgres-in-WASM) | process memory |
| Deployed web | Postgres | `DATABASE_URL` |
| `copypaste serve` | SQLite WAL | `~/.copypaste/pastes.sqlite` |

Rows are unowned. There is no user table and no listing query.

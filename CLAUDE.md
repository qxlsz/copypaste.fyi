# CLAUDE.md: copypaste.fyi

copypaste.fyi is a Rust and React paste service with controlled writes, server-side authenticated
encryption, metadata-only moderation, optional Upstash REST persistence, and an OCaml verifier.

## Security facts

- Encryption is server-side. Require TLS from the client to a trusted edge; plaintext and the
  supplied key may then traverse the edge-to-Rocket hop and exist in process memory. Keys are not
  intentionally persisted. Never describe this as client-side, end-to-end, or zero-knowledge
  encryption.
- For AES-256-GCM and ChaCha20-Poly1305 creation, Rust forwards the plaintext, supplied key,
  ciphertext, nonce, and salt to the OCaml verifier. Fly carries that request over application-layer
  HTTP on its private network to a separate VM. Treat the verifier and private transport as part of
  the plaintext trust boundary.
- Closed deployments use `X-CopyPaste-Write-Token` for service admission. `Authorization` may carry
  optional signed-session identity on creation. Live updates use the ownership token in
  `Authorization` and service admission in `X-CopyPaste-Write-Token`.
- The create guard retains a legacy fallback that treats a non-session service credential in
  `Authorization` as admission. Do not extend or advertise it as the current client contract.
  Mutation guards have no fallback and require the dedicated admission header.
- Bundles, attestations, webhooks, and steganography are disabled in the hardened build. Requests
  are rejected. Setting the former feature allow flags to `true` stops startup.
- Burn-after-reading, mutation ordering, and delete protection are serialized only within one app
  process. Redis does not provide a distributed consume, version, or tombstone here.
- Sessions, challenges, user/workspace listings, statistics, and cached paste IDs are process-local.
  Operate one app instance until those states are shared.
- Admin moderation never returns content or encryption material. Audit records log only admin key
  ID, action, and outcome. They include no paste identifier, derived target, or access count.

## Stack

- Rust 2021, Rocket 0.5.1, Tokio
- React 19, Vite 7, TypeScript, Tailwind CSS, Zustand, TanStack Query, Monaco Editor
- OCaml 5.2, `mirage-crypto`, Cohttp/Lwt
- Optional Solidity anchor contract reached through an HTTP relayer
- Distroless Docker runtime, Docker Compose, and Fly.io

The Fly `app` and `crypto-verifier` process groups run on separate VMs. The app reaches the verifier
through Fly private DNS, not localhost.

## Run

```bash
# Backend
cargo build
ROCKET_ADDRESS=127.0.0.1 cargo run --bin copypaste -- serve

# Frontend
cd frontend
npm install
npm run dev

# Both development processes
ROCKET_ADDRESS=127.0.0.1 ./scripts/run_both.sh

# Container stack
docker compose up --build
```

The only Rust binary is `src/bin/copypaste.rs`. It has `serve`, `send`, and `config init`
subcommands. Do not document starting it without `serve`. The backend configuration default is
`0.0.0.0`; local examples must set loopback explicitly.

## Test

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo llvm-cov --workspace --all-features --nextest --fail-under-lines 75

cd frontend
npm run lint
npm test -- --run
```

Use `./scripts/precommit.sh` for the full local pre-commit workflow.

## Layout

```text
src/lib.rs               Core types, PasteStore, process cache, persistence boundary
src/bin/copypaste.rs     serve, send, and config init CLI
src/server/api_keys.rs   Static credentials, SQLite dynamic keys, write/admin guards
src/server/handlers.rs   Rocket routes, quarantine, feature policy, moderation
src/server/crypto.rs     Encryption, decryption, verifier client
src/server/redis.rs      Upstash HTTPS REST persistence adapter
src/server/tor.rs        Exact-host and trusted-ingress Tor decision
src/server/blockchain.rs Content-safe anchor manifest and relayer
src/server/sessions.rs   Process-local challenges and sessions
frontend/                React SPA
ocaml-crypto-verifier/   AES-GCM, ChaCha20-Poly1305, and Ed25519 verifier
blockchain/              Optional anchor contract
tests/                   Rust integration tests
docs/                    Operator and encryption guides
```

## Routes

Pastes:

- `POST /api/pastes`: create JSON response
- `POST /`: legacy create response
- `GET /api/pastes/{id}`: JSON read
- `GET /p/{id}`: canonical share route
- `GET /{id}`: legacy HTML route
- `GET /raw/{id}`: raw text route
- `PUT /api/pastes/{id}`: update a live paste
- `PATCH /api/pastes/{id}/finalize`: finalize a live paste
- `POST /api/pastes/{id}/anchor`: admin-only manifest anchor

Authentication and indexes:

- `GET /api/auth/challenge`, `POST /api/auth/login`, `POST /api/auth/logout`
- `GET /api/user/paste-count`, `GET /api/user/pastes`
- `GET /api/workspaces/{name}/pastes`

Operations:

- `GET /health`, `GET /api/health`: public liveness only; neither probes storage nor verifier
- `GET /api/stats/summary`: process-local cache statistics
- `GET /api/openapi.json`: self-contained OpenAPI JSON
- `POST|GET|DELETE /api/admin/keys[/{id}]`: dynamic key management, available only with secure
  SQLite configuration
- `GET|DELETE /api/admin/pastes/{id}`: exact-ID metadata inspection and deletion

The JSON paste route returns `423 Locked` before and after a configured time-lock window. Raw reads
return `423` before the window and `410` after it. Retention expiry returns `410` only while the
expired record remains observable; a removed cache entry or elapsed Redis TTL yields `404`.
All JSON, HTML, and raw read routes accept `X-Paste-Key`; it overrides legacy `?key=`.

## Storage

`COPYPASTE_PERSISTENCE_BACKEND` accepts only:

- `memory` or unset: process memory only
- `redis`: Upstash REST using `UPSTASH_REDIS_REST_URL` and
  `UPSTASH_REDIS_REST_TOKEN`

`COPYPASTE_REDIS_KEY_PREFIX` defaults to `paste:`. The Redis adapter uses JSON-body commands over a
clean HTTPS origin (loopback HTTP is allowed for tests), disables redirects, applies timeouts,
bounds responses, and puts no record data in URL paths. Explicit backend initialization failure
stops startup; it never falls back to memory.

The process cache is authoritative for indexes and statistics. Redis is loaded only by exact ID.
User/workspace dashboards therefore do not enumerate the full Redis dataset after restart or on a
different app instance.

Creates, updates, and finalization persist before mutating the cache. Deletes remove persistent
state before local cache state. Per-ID locks prevent local reorder and local update-after-delete
resurrection, but not cross-instance races. Keep a reported ID quarantined and roll every instance
before deleting it.

## Authentication

Static credentials:

- `COPYPASTE_AUTH_TOKEN`: service write admission
- `COPYPASTE_ADMIN_TOKEN`: bootstrap admin and write admission
- `COPYPASTE_REQUIRE_WRITE_AUTH=true`: closes writes even if the static write token is missing
- `COPYPASTE_ALLOW_SESSION_WRITES=false`: current hardened policy

In production, each non-empty static token must be 43 to 128 base64url characters. Invalid values
stop startup. Generate at least 256 random bits and retain the administrator token in an approved
password manager before importing it into the deployment secret manager.

Dynamic API keys require `COPYPASTE_SQLITE_PATH`. On Unix, the parent directory must already exist,
belong to the service user, and be mode `0700`; the database is owner-only mode `0600`. An unsafe
explicit path stops startup. Other platforms need equivalent operator-managed owner-only ACLs; the
app does not verify Unix ownership or modes there. With no path, static tokens work and dynamic key
endpoints return `503`.

The checked-in Fly configuration does not set a SQLite path. It relies on the retained static admin
token. A local SQLite file is not shared between Fly VMs.

Login challenges are random, short-lived, stored server-side, and consumed atomically. Challenges
and 24-hour login sessions disappear on restart and are not shared between app instances.

## Tor

`COPYPASTE_ONION_HOST` and `COPYPASTE_ONION_INGRESS_TOKEN` must be configured together. The token
must be 32 to 512 visible-ASCII bytes. An incomplete or invalid pair stops startup.

`OnionAccess` trusts a request only when the actual `Host` exactly matches the configured onion
hostname and `X-Copypaste-Onion-Ingress` matches the configured secret in constant time. It never
uses `X-Forwarded-Host`. The proxy must strip every client-provided ingress header and inject the
secret only for the onion listener.

## Encryption and verification

Standard symmetric modes derive 32 bytes with `SHA-256(salt || passphrase)` and a random 16-byte
salt. The supported storage algorithms are AES-256-GCM, ChaCha20-Poly1305,
XChaCha20-Poly1305, and an experimental ML-KEM-768 plus AES-256-GCM envelope.

The OCaml service independently supports AES-256-GCM, ChaCha20-Poly1305, and Ed25519 signatures. It
does not support XChaCha20-Poly1305 or ML-KEM. The Rust encryption path sends only supported AES and
ChaCha creation operations for dual verification. Those requests contain plaintext, the supplied
key, ciphertext, nonce, and salt. On Fly they cross private application-layer HTTP to the verifier's
separate VM, so both that VM and private network belong to the plaintext trust boundary. With
`COPYPASTE_REQUIRE_CRYPTO_VERIFICATION=true`, any network, HTTP, parsing, size, or `valid: false`
failure for those supported operations fails creation. The checked-in Fly and Compose deployments
enable strict mode.

Read `docs/encryption.md` before changing the envelope or verifier contract.

## Anchoring

Anchoring is admin-only and requires `ANCHOR_RELAY_ENDPOINT`. Plaintext manifests contain metadata
but no plaintext hash or content commitment. Encrypted manifests add a domain-separated digest of
the randomized encrypted storage tuple. Do not describe a plaintext anchor as proof of content.

## Configuration notes

- Maximum paste content is capped at 1 MiB.
- `--config`, `COPYPASTE_CONFIG`, `./copypaste.toml`, and `/etc/copypaste/server.toml` are checked in
  that order.
- The config bridge maps an Upstash `storage.url` to `UPSTASH_REDIS_REST_URL`; the REST token must
  still come from the environment or secret manager.
- The config auth token is consumed as `COPYPASTE_AUTH_TOKEN`. Clients send it in
  `X-CopyPaste-Write-Token`.
- Public rate limits are per process. Shared edge controls remain required.
- Public health is liveness-only. Monitor Redis and the verifier independently.

## CLI secret handling

`copypaste send` accepts service credentials from `--auth-token-file` or
`COPYPASTE_AUTH_TOKEN`. It accepts encryption keys from `--encryption-key-file` or
`COPYPASTE_ENCRYPTION_KEY`. It never accepts either secret in argv. Secret files must be owner-only
on Unix.

## Development conventions

- Install hooks with `./scripts/setup_git_hooks.sh`.
- Use `rg` for repository search.
- Preserve user changes in a dirty worktree.
- Use `apply_patch` for hand edits.
- Add tests for security policy and error mapping changes.
- See `agents.md`, `SECURITY.md`, `docs/abuse-response.md`, and `docs/encryption.md` for the extended
  project rules.

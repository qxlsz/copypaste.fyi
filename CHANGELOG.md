# Changelog

All notable changes to copypaste.fyi are documented here. The project follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and semantic versioning.

## [Unreleased]

### Added

- Added `copypaste healthcheck` so distroless images can probe `GET /api/health` in Docker exec
  form without a shell or curl.
- Added exact-ID, metadata-only moderation at `GET /api/admin/pastes/{id}` and targeted deletion at
  `DELETE /api/admin/pastes/{id}`. The moderation response excludes paste content, ciphertext,
  keys, nonces, salts, raw workspace labels, attestation secrets, ownership tokens, and webhook
  targets.
- Added a deployment quarantine through `COPYPASTE_BLOCKED_PASTE_IDS`. The complete configured set
  blocks JSON, share, legacy HTML, raw, update, finalize, and anchor routes before public storage
  access while leaving administrator metadata triage available.
- Added content-free moderation audit events containing only the admin key ID, action, and outcome.
  Raw paste IDs, derived targets, and access counts are not written to those audit records.
- Added the operator [abuse-response runbook](docs/abuse-response.md), including authoritative-list
  handling, credential retention, no-content triage, full-instance rollout, escalation, and
  cross-instance deletion limits.
- Added file-backed dynamic API keys through `COPYPASTE_SQLITE_PATH`. On Unix, the parent directory
  must be owner-controlled mode `0700`; the SQLite file is mode `0600`, stores Argon2id hashes, and
  fails startup when an explicitly configured path is unsafe. Other platforms require equivalent
  operator-managed owner-only ACLs.
- Added the canonical `/p/{id}` share route and `/raw/{id}` raw route while retaining the legacy
  `/{id}` HTML route.
- Added CLI `serve`, `send`, and `config init` subcommands. `send` reads write credentials from
  `--auth-token-file` or `COPYPASTE_AUTH_TOKEN` and encryption keys from
  `--encryption-key-file` or `COPYPASTE_ENCRYPTION_KEY`.

### Fixed

- Docker and Compose now detect an unresponsive `copypaste` process. The distroless image and
  Compose service probe `GET /api/health` through `copypaste healthcheck` (exec form, no shell).

### Security

- Rate-limited responses now include `Retry-After: 60` and expose that header to allowlisted CORS
  origins so clients can wait out the fixed 60-second window.
- Introduced `X-CopyPaste-Write-Token` to separate service admission from optional signed-session
  identity in `Authorization`. Creation retains a legacy service-credential fallback in
  `Authorization`; current clients use the dedicated header. Live mutations require service
  admission and the paste ownership token in separate headers.
- Enforced 43 to 128 base64url characters for non-empty production
  `COPYPASTE_AUTH_TOKEN` and `COPYPASTE_ADMIN_TOKEN` values. Invalid static credentials now stop
  startup; operators should generate at least 256 random bits before encoding.
- Configured the supplied Fly deployment to require write admission, reject session-only writes,
  use strict supported-algorithm verification, and use the Redis persistence backend.
- Kept `COPYPASTE_ADMIN_TOKEN` as the bootstrap moderation credential. The current Fly
  configuration has no SQLite path, so dynamic key-management endpoints fail closed with `503`
  instead of creating process-only credentials.
- Made issued login challenges short-lived, bounded, server-stored, single-use, and atomically
  consumed. Signed but unissued or replayed challenges are rejected.
- Replaced enumerable paste identifiers with CSPRNG-backed 24-character identifiers carrying 144
  bits of entropy. Existing IDs remain readable.
- Added strict paste-ID validation for quarantine and moderation inputs. Invalid quarantine
  configuration stops startup.
- Changed configured persistence to fail startup when initialization fails. The server no longer
  silently falls back to memory after an explicit backend selection.
- Changed durable creates, updates, finalization, and deletes to complete the backing-store write
  before publishing the matching cache change or acknowledging success. Storage load failures now
  surface as `503` rather than `404`.
- Serialized update, finalize, delete, and cache-fill operations per paste ID within one process.
  This prevents local reorder, reopening after local finalization, and local in-flight save
  resurrection after delete.
- Changed the Upstash adapter to send `SET`, `SETEX`, `GET`, and `DEL` as JSON command arrays in POST
  bodies with bearer authentication. Serialized records and content never appear in URL paths.
- Restricted the Upstash origin to clean HTTPS, with loopback HTTP only for local use. Redirects are
  disabled; connect/request timeouts and bounded response reads are enforced; provider response
  bodies are not reflected in public errors.
- Added exact Tor ingress authentication. Tor-only access requires both the configured onion
  `Host` and a constant-time match on `X-Copypaste-Onion-Ingress`. The onion host and token must be
  configured as a valid pair, or startup fails. `X-Forwarded-Host` is never trusted.
- Reduced public health responses to versioned liveness data. They do not expose dependency state,
  counts, internal URLs, or upstream errors.
- Sanitized server-rendered Markdown; tightened CORS, security headers, cache controls, crawler
  directives, request sizes, and process-local rate limits.
- Stopped loading the code editor from a third-party CDN and removed the third-party interactive API
  documentation runtime. The self-contained OpenAPI JSON remains available.
- Replaced frontend `Math.random()` key generation with 256-bit Web Crypto keys. The frontend removes
  legacy query-string keys from navigation and uses URL fragments plus `X-Paste-Key`. The backend
  query parameter remains only for compatibility.
- Made CLI requests reject redirects, require HTTPS for non-loopback origins, bound response sizes,
  and print key-free share links. The CLI does not accept credentials or encryption keys in argv.
- Added Rust and npm security-audit checks to CI.

### Changed

- Raised the documented and CI Rust toolchain floor to 1.88 to match the resolved dependency set.
- Corrected the product security model: encryption runs in the Rust service. Clients must use TLS
  to a trusted edge, but plaintext and the supplied key may traverse the edge-to-Rocket hop and
  exist in memory. Keys are not intentionally persisted, and the service is not browser-only,
  end-to-end, or zero-knowledge.
- Limited the supported paste persistence backends to process memory and Upstash Redis REST. Redis
  is selected with `COPYPASTE_PERSISTENCE_BACKEND=redis` and configured with
  `UPSTASH_REDIS_REST_URL` plus `UPSTASH_REDIS_REST_TOKEN`.
- Changed the JSON time-lock response to `423 Locked` while outside the configured access window.
  An observable expired record returns `410 Gone`; after cache removal or Redis TTL expiry, it is
  indistinguishable from an absent record and returns `404`.
- Restricted anchoring to administrators and removed plaintext content commitments. Plaintext
  manifests contain content-free metadata only. Encrypted manifests commit, with domain
  separation, to randomized encrypted storage fields rather than deterministic plaintext hashes.
- Clarified verifier coverage. The OCaml service supports AES-256-GCM,
  ChaCha20-Poly1305, and Ed25519 signatures. XChaCha20-Poly1305 and ML-KEM remain Rust-only. Strict
  mode fails supported encryption creation on verifier transport, HTTP, parsing, size, or
  validation failure.
- Documented that supported encryption verification sends plaintext, the supplied key, ciphertext,
  nonce, and salt to the verifier. On Fly, this application-layer HTTP request crosses the private
  network to a separate VM, which expands the plaintext trust boundary.
- Configured Fly `app` and `crypto-verifier` as separate process-group VMs connected through private
  DNS. They do not share localhost or a filesystem.
- Documented `/health` and `/api/health` as liveness only. Storage and verifier readiness require
  separate monitoring.

### Disabled

- Disabled bundles, attestations, webhooks, and steganography in the hardened build. New requests
  for these features are rejected. Setting former allow flags to `true` is unsupported and stops
  startup; bundles have no enable flag.
- Disabled client-selected external paste persistence. Storage is controlled by the deployment.

### Known limitations

- Burn-after-reading is best-effort. Link previews can consume a paste, and concurrent app instances
  can read the same record before deletion. The current Redis path does not provide an atomic
  cross-instance consume.
- Per-ID mutation locks are process-local. A stale app instance can save an older live record after
  another instance deletes it. The service has no distributed version or tombstone.
- Administrator deletion confirms only the handling instance and its configured backing-store
  request. It cannot prove that another instance has no cached copy or in-flight save. Operators
  must publish the authoritative quarantine set, roll every app instance before deletion, and keep
  the target quarantined.
- Sessions and challenges are process-local and disappear on restart. User/workspace listings,
  statistics, and cached paste-ID indexes do not enumerate the full Redis dataset. Run one app
  instance until these states are shared.
- Per-IP limits are process-local. Public deployments still need shared edge quotas, bot controls,
  a monitored abuse channel, and qualified legal review.
- The experimental ML-KEM/passphrase envelope has not received independent cryptographic review or
  FIPS validation. Low-entropy passphrases remain vulnerable to offline guessing.
- End-to-end browser encryption is not implemented. The server sees plaintext and keys during
  encryption and decryption.

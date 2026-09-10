# Changelog

All notable changes to copypaste.fyi are documented here. The project follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and semantic versioning.

## [Unreleased]

### Changed

- Public meta description no longer claims steganography (disabled in the hardened build). Added dns-prefetch/preconnect to the Fly API host for faster first paste create.

### Added

- Self-host helper on About now has a one-tap Copy for the recipe commands.
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

- `copypaste send --json` / `--agent` receipts now map a legacy `/{id}` create path to
  `GET /api/pastes/{id}` so the `get` field is the JSON read URL, not the HTML page.
- Self-host cookbook now uses `--auth-token-file` (the real CLI flag) instead of
  `--write-token-file`.
- Docker and Compose now detect an unresponsive `copypaste` process. The distroless image and
  Compose service probe `GET /api/health` through `copypaste healthcheck`.

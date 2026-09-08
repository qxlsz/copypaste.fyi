# Changelog

All notable changes to copypaste.fyi are documented here. The project follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and semantic versioning.

## [Unreleased]

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
  Compose service probe `GET /api/health` through `copypaste healthcheck` (exec form, no shell).

### Security

- Rate-limited responses now include `Retry-After: 60` and expose that header to allowlisted CORS
  origins so clients can wait out the fixed 60-second window.
- Introduced `X-CopyPaste-Write-Token` to separate service admission from optional signed-session
  identity in `Authorization`. Creation retains a legacy service-credential fallback in
  `Authorization`; current clients use the dedicated header. Live mutations require service
  admission and the paste ownership token in separate headers.

## [0.2.0] - 2026-09-05

### Added

- Homebrew formula with release tarball sha256s for darwin/linux arm64/x64.
- Share image card with QR, short paste id, burn and encrypt marks.
- Composer box: drop a file, paste from clipboard, tab indent, live size.
- Faster first paint: split Monaco, defer QR, slimmer fonts.
- Self-host helper picks Grok VM, Cursor, or brew.
- Cloud Agent development environment and agent-setup.sh.

### Fixed

- Various create/read/CORS contract tests and send receipt get URL.
- Share key leak, zero retention, and burn/expiry gaps.

### Changed

- Public site remains anonymous; FORCE_MEMORY and no write auth on Fly.

## Notes

- Paste content is never written to moderation audit events.
- Server-side encryption only; not E2E or zero-knowledge.
- Sessions and challenges are process-local and disappear on restart. User/workspace listings,
  statistics, and cached paste-ID indexes do not enumerate the full Redis dataset. Run one app
  instance until these states are shared.
- Per-IP limits are process-local. Public deployments still need shared edge quotas, bot controls,
  a monitored abuse channel, and qualified legal review.
- The experimental ML-KEM/passphrase envelope has not received independent cryptographic review or
  FIPS validation. Low-entropy passphrases remain vulnerable to offline guessing.
- End-to-end browser encryption is not implemented. The server sees plaintext and keys during
  encryption and decryption.

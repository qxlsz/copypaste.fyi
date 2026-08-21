# Security policy

## Supported versions

| Version | Supported |
|---|---|
| `main` branch | Yes |
| Latest tagged release | Yes |
| Older tagged releases | No |

## Report a vulnerability

Do not open a public GitHub issue for a suspected vulnerability. Submit a private
[GitHub Security Advisory](https://github.com/qxlsz/copypaste.fyi/security/advisories/new) with:

1. The affected version or commit
2. The security impact
3. Reproduction steps or a proof of concept
4. Any mitigation you have tested

The maintainers aim to acknowledge reports within 72 hours. Confirmed issues receive priority based
on impact and exploitability. We will coordinate disclosure and request a CVE when appropriate.

Do not include real illegal content, production credentials, or unrelated user data in a security
report.

## Report abusive or illegal content

Vulnerability reports and content reports follow different processes. Do not download, attach,
copy, or re-upload suspected illegal material. Send the exact copypaste.fyi URL or paste ID, the
provider case number if present, and the time observed through the deployment's monitored abuse
channel.

Operators must prepare and test the [abuse-response runbook](docs/abuse-response.md) before
accepting public writes. The runbook uses exact-ID quarantine, metadata-only triage, and targeted
deletion without asking an operator to view content. Suspected child sexual abuse material requires
immediate escalation through qualified counsel and the applicable reporting process.

## Deployment requirements

Treat the following as production requirements, not optional hardening:

- Close write admission. Set `COPYPASTE_REQUIRE_WRITE_AUTH=true` and have current clients supply a
  service credential in `X-CopyPaste-Write-Token`. `Authorization` may carry optional
  signed-session identity, but the supplied Fly policy does not treat a session as admission.
- Keep `COPYPASTE_ADMIN_TOKEN` in an approved password manager and deployment secret manager.
  Retain the password-manager record so operators can moderate and rotate it. The current Fly
  configuration uses this static token.
- Generate both static application tokens from at least 256 random bits encoded as 43 to 128
  base64url characters. A malformed non-empty `COPYPASTE_AUTH_TOKEN` or
  `COPYPASTE_ADMIN_TOKEN` stops production startup.
- Configure dynamic API keys only with a secure `COPYPASTE_SQLITE_PATH`. On Unix, the parent
  directory must be owner-controlled mode `0700`, and the database is mode `0600`. On other
  platforms, enforce equivalent owner-only ACLs; the app does not verify Unix ownership or modes
  there. No path means dynamic key management is disabled.
- Use `COPYPASTE_PERSISTENCE_BACKEND=redis` with `UPSTASH_REDIS_REST_URL` and
  `UPSTASH_REDIS_REST_TOKEN` for supported durable paste storage. Explicit initialization failure
  stops startup rather than falling back to memory.
- Run one app instance. The verifier can run in its separate process group, but app sessions,
  listings, statistics, caches, rate limits, burn behavior, and mutation ordering are not
  distributed.
- Put public writes behind shared edge quotas, request-size enforcement, bot controls, and a
  monitored abuse channel. Application limits are process-local backstops.
- Keep bundles, attestations, webhooks, and steganography disabled. The hardened build rejects them;
  setting former feature allow flags to `true` is unsupported and stops startup.
- Treat `/health` and `/api/health` as liveness only. They do not check Redis, the verifier, or the
  anchor relayer. Monitor those dependencies independently.

## Tor ingress boundary

Tor-only pastes are trusted only when both conditions match:

1. The actual `Host` is the exact configured `COPYPASTE_ONION_HOST`.
2. `X-Copypaste-Onion-Ingress` matches the secret configured in
   `COPYPASTE_ONION_INGRESS_TOKEN`.

The pair is mandatory and invalid configuration stops startup. The ingress token must be 32 to 512
visible-ASCII bytes. The application compares its digest in constant time and does not trust
`X-Forwarded-Host`.

Every edge must remove any client-supplied onion-ingress header. Only the trusted onion listener may
inject the configured value, and it must set or preserve the exact onion host. A `Host` header by
itself is never proof of Tor ingress.

## Cryptographic architecture

copypaste.fyi currently performs encryption in the Rust service:

- Require TLS from the client to a trusted edge. Plaintext and the supplied key may then traverse
  the edge-to-Rocket hop, and the service sees both in memory. It does not intentionally persist the
  supplied key. This is not client-side, end-to-end, or zero-knowledge encryption.
- AES-256-GCM, ChaCha20-Poly1305, and XChaCha20-Poly1305 derive a 32-byte key with
  `SHA-256(salt || passphrase)` and a random 16-byte salt. A single SHA-256 operation does not
  harden low-entropy passwords against offline guessing.
- The experimental ML-KEM-768 plus AES-256-GCM envelope derives a deterministic ML-KEM keypair from
  the supplied key and uses fresh encapsulation randomness. The composition has not received an
  independent review or FIPS validation.
- The OCaml service supports independent AES-256-GCM and ChaCha20-Poly1305 verification plus an
  Ed25519 signature endpoint. It does not support XChaCha20-Poly1305 or ML-KEM.
- AES and standard ChaCha creation verification sends plaintext, the supplied key, ciphertext,
  nonce, and salt to the OCaml service. Fly carries this over application-layer HTTP on its private
  network to a separate VM. The verifier VM and private transport are therefore inside the trusted
  plaintext boundary.
- With `COPYPASTE_REQUIRE_CRYPTO_VERIFICATION=true`, any verifier network, HTTP, parsing, size, or
  `valid: false` failure blocks supported AES or ChaCha creation. The checked-in Fly and Compose
  deployments enable this strict mode.

Read [Encryption guide](docs/encryption.md) for the exact envelope and CLI handling.

## Anchor privacy

The anchor endpoint requires administrator authentication. A plaintext anchor contains only
content-free metadata and does not commit to or prove the plaintext. An encrypted anchor includes a
domain-separated digest of randomized encrypted storage fields. It does not publish the supplied
key or a deterministic hash of short plaintext.

## Known limitations

- Burn-after-reading is best-effort. Link previews can consume a paste. Concurrent app instances can
  both read before either deletion completes because the Redis adapter does not use an atomic
  consume operation.
- Per-ID locks prevent local update/finalize/delete reorder, but they do not coordinate different
  app instances. A stale instance can save an old live record after another instance deletes it,
  resurrecting persistent data.
- Administrator deletion confirms the handling instance and its configured backing-store request.
  It cannot prove that another instance has no stale cache or in-flight save. Quarantine the full
  exact-ID set and roll every app instance before deletion, then keep the target quarantined.
- Login challenges and sessions are process-local and disappear on restart. User/workspace
  dashboards and statistics scan only IDs in the local cache, not the full Redis dataset.
- Redis persistence does not make multi-instance operation correct. Until shared sessions, indexes,
  atomic burn, and distributed mutation versioning exist, run a single app instance.
- JSON, HTML, and raw reads still accept decryption keys in query strings for compatibility. All
  three also accept `X-Paste-Key` with header precedence. New clients must use the header; URLs can
  leak through histories, referrers, logs, screenshots, and sync.
- Metadata-only moderation can remove a reported exact ID, but there is no bulk content browser,
  content classifier, or master decryption key. Automated controls cannot prove that an upload is
  lawful.
- Public health is not readiness. An `ok` response does not establish Redis durability or verifier
  availability.
- Paste creation retains a compatibility fallback that accepts a non-session service credential in
  `Authorization`. New clients must use `X-CopyPaste-Write-Token` so `Authorization` remains
  available for identity. Live mutations require the dedicated admission header.

# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| `main` branch | Yes |
| Tagged releases | Yes (latest tag only) |

## Reporting a vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please report security issues via [GitHub Security Advisories](https://github.com/qxlsz/copypaste.fyi/security/advisories/new).
This keeps the report private until a fix is ready. Include:

1. A description of the vulnerability and its potential impact
2. Steps to reproduce or a proof-of-concept
3. The version/commit hash you tested against
4. Any suggested mitigations you have identified

You will receive an acknowledgement within 72 hours. If the vulnerability is confirmed, a fix will
be prioritized and a CVE will be requested if the severity warrants it. You will be credited in the
release notes unless you prefer to remain anonymous.

## Cryptographic architecture

copypaste.fyi uses a defense-in-depth approach to cryptography:

- **Primary implementation**: Rust (`aes-gcm`, `chacha20poly1305`, `pqc_kyber`, `sha2`, `ed25519-dalek`)
- **Independent verification**: OCaml service using `mirage-crypto` (port 8001) — every encrypt/decrypt
  operation is cross-checked by this independent implementation before the result is accepted
- **Client-side encryption**: Passphrases and derived keys are never stored server-side
- **Key derivation**: 16-byte random salt + SHA-256 hash per paste; salts are stored alongside ciphertext

Supported algorithms: AES-256-GCM, ChaCha20-Poly1305, XChaCha20-Poly1305, Kyber hybrid AES-256-GCM.

See [`docs/encryption.md`](docs/encryption.md) for full algorithm and key derivation details.

## Known limitations

- The Kyber hybrid implementation currently uses SHA-256 as a KEM simulation rather than a certified
  Kyber-1024 library. It provides post-quantum *structure* but not NIST-certified PQC security.
  A migration to `pqcrypto` crate is planned.
- There is no built-in rate limiting. Public deployments should be placed behind an authenticated
  reverse proxy or a rate-limiting layer (nginx `limit_req`, Caddy rate-limit plugin, etc.).
- In-memory storage means paste loss on restart. Redis persistence is available via
  `COPYPASTE_REDIS_URL` but is not enabled by default.

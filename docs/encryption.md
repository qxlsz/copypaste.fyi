# Encryption guide

This guide describes the current server-side encryption boundary, storage formats, verifier
coverage, key sharing, and CLI secret handling.

## Trust model

copypaste.fyi performs encryption and decryption in the Rust service. Require TLS from the browser
or CLI to a trusted edge. Plaintext and the supplied key may then traverse the edge-to-Rocket hop,
and the service sees both in process memory. It does not intentionally persist the key.

This design is not browser-only, end-to-end, or zero-knowledge encryption. Users who do not trust
the service with plaintext must encrypt locally before upload and treat the result as an ordinary
opaque paste.

For AES-256-GCM and ChaCha20-Poly1305 creation, the Rust service forwards the plaintext, supplied
key, ciphertext, nonce, and salt to the OCaml verifier. On Fly, this uses application-layer HTTP
over the private network to a separate VM. The verifier VM and private transport are therefore part
of the trusted plaintext boundary. XChaCha20-Poly1305 and ML-KEM operations stay in Rust.

The stored record contains ciphertext, nonce, salt or envelope fields, and an algorithm identifier.
It does not contain the supplied key.

## Standard symmetric key derivation

AES-256-GCM, ChaCha20-Poly1305, and XChaCha20-Poly1305 use this derivation:

1. Generate a random 16-byte salt with the operating-system CSPRNG.
2. Compute `SHA-256(salt || supplied_key)`.
3. Use the 32-byte digest as the symmetric key.
4. Store the salt with the ciphertext so the key can be derived again for decryption.

The salt prevents one precomputed table from applying unchanged to every paste. A single SHA-256
operation is not a password-hardening KDF, so low-entropy human passphrases remain vulnerable to
offline guessing after ciphertext disclosure. Prefer the web application's random 256-bit keys or
another high-entropy secret source.

## Supported storage algorithms

| Algorithm | API identifier | Nonce | Independent OCaml encryption check |
|---|---|---:|---|
| AES-256-GCM | `aes256_gcm` | 12 random bytes | Yes |
| ChaCha20-Poly1305 | `chacha20_poly1305` | 12 random bytes | Yes |
| XChaCha20-Poly1305 | `xchacha20_poly1305` | 24 random bytes | No, Rust only |
| ML-KEM-768 plus AES-256-GCM | `kyber_hybrid_aes256_gcm` | 12 random AES bytes | No, Rust only |

All four paths use authenticated encryption. An authentication failure returns an invalid-key
error and no plaintext.

### AES-256-GCM

The `aes-gcm` implementation generates a fresh 96-bit nonce per encryption. This is the most broadly
interoperable mode in the project and is covered by the OCaml verifier.

### ChaCha20-Poly1305

The `chacha20poly1305` implementation generates a fresh 96-bit nonce per encryption. It is also
covered by the OCaml verifier.

### XChaCha20-Poly1305

The XChaCha implementation uses a fresh 192-bit nonce. `mirage-crypto` does not expose the required
XChaCha20/HChaCha20 operation, so this mode is verified only by Rust. Strict verifier mode does not
add an independent check for it.

### Experimental ML-KEM-768 envelope

The `kyber_hybrid_aes256_gcm` identifier is retained for API compatibility, but the implementation
uses ML-KEM-768:

1. HKDF derives deterministic ML-KEM-768 key-generation seeds from the supplied key.
2. ML-KEM encapsulation uses fresh operating-system randomness to produce a KEM ciphertext and
   shared secret.
3. HKDF derives an AES-256-GCM key from the shared secret.
4. AES-256-GCM encrypts the paste with a fresh 96-bit nonce.
5. The record stores `kem_ciphertext|aes_ciphertext|aes_nonce`. The decapsulation key is re-derived
   from the supplied key and is not stored.

The underlying ML-KEM primitive is real. The passphrase-derived composition around it has not
received independent cryptographic review or FIPS validation. Low-entropy supplied keys still
permit offline guessing. Treat the mode as experimental. The Rust decoder retains compatibility
with older envelope records.

## Independent verifier

The OCaml verifier supports:

- AES-256-GCM encryption verification
- ChaCha20-Poly1305 encryption verification
- Ed25519 signature verification through its separate signature endpoint

The Rust encryption path sends AES and standard ChaCha creation operations, including plaintext,
the supplied key, ciphertext, nonce, and salt, to the verifier after it encrypts. It does not send
XChaCha or ML-KEM operations because the verifier cannot validate them.
The login implementation verifies Ed25519 in Rust; do not imply that every login uses the OCaml
signature endpoint.

`COPYPASTE_REQUIRE_CRYPTO_VERIFICATION=true` enables strict behavior for supported AES and ChaCha
creation. A verifier connection, timeout, redirect, HTTP status, response-size, parsing, or
`valid: false` failure rejects the operation. XChaCha and ML-KEM remain Rust-only even in strict
mode. The checked-in Docker Compose and Fly configurations enable strict mode.

The verifier client accepts a clean HTTPS origin, or HTTP only on loopback and Fly private
`.internal` ingress. Fly's checked-in verifier URL uses HTTP, not application-layer TLS. The client
disables redirects, uses short timeouts, and bounds response bodies. The backend's `/health` and
`/api/health` routes do not probe the verifier; monitor the verifier separately.

## Web key generation and sharing

The web application generates 32 random bytes with `crypto.getRandomValues()` and encodes them as
base64url.

Share the key separately from the key-free `/p/{id}` URL when possible. A URL fragment is not sent
in the HTTP request, but browser history and sync, extensions, screenshots, QR images, native share
targets, and clipboard managers can still expose the combined link.

For API reads, send the key in `X-Paste-Key`:

```http
GET /api/pastes/{id} HTTP/1.1
Host: your-instance.example
X-Paste-Key: key-from-a-secret-source
```

The JSON, HTML, and raw read routes all accept `X-Paste-Key`, and the header takes precedence. The
backend retains `?key=` on those routes for compatibility. New sensitive automation should use the
header, never a query-string key.

## CLI secret handling

The CLI is the `copypaste send` subcommand. It never accepts authentication tokens or encryption
keys as command arguments.

Read a key from an owner-only file:

```bash
chmod 600 ./paste-key
copypaste send \
  --encryption-mode chacha20_poly1305 \
  --encryption-key-file ./paste-key \
  "fn main() {}"
```

An isolated supervisor or secret manager may instead inject `COPYPASTE_ENCRYPTION_KEY` into the
client process environment without exposing its value in a command.

Closed deployments also require `--auth-token-file` or `COPYPASTE_AUTH_TOKEN`. The CLI sends that
credential in `X-CopyPaste-Write-Token`:

```bash
chmod 600 ./write-token
copypaste send --auth-token-file ./write-token "note"
```

`--encryption-mode` accepts `none`, `aes256_gcm`, `chacha20_poly1305`, and
`xchacha20_poly1305`. The CLI does not expose the experimental ML-KEM mode. It prints a key-free
share URL; retain and distribute the key separately.

## Persistence

Memory storage loses records on restart. The supported persistent option uses:

```text
COPYPASTE_PERSISTENCE_BACKEND=redis
UPSTASH_REDIS_REST_URL=https://your-database.upstash.io
UPSTASH_REDIS_REST_TOKEN=your-provider-token
```

The Upstash adapter sends serialized records in HTTPS JSON request bodies, applies Redis TTLs, and
never places record data in URL paths. Explicit configuration failure stops startup. Persistence
protects availability across restart; it does not change the encryption trust boundary because the
Rust service still receives the supplied key and plaintext.

The app also keeps a process-local cache. Redis does not provide atomic cross-instance burn,
distributed mutation versions, or deletion tombstones in this implementation. Operate one app
instance. A cross-instance stale update can resurrect a deleted record, so incident response must
quarantine and roll every app instance before deletion.

## Anchor behavior

The admin-only anchor manifest treats plaintext and encrypted records differently:

- Plaintext produces content-free metadata only. It has no content digest and cannot prove the
  plaintext that existed.
- Encrypted content adds a domain-separated digest of randomized encrypted storage fields,
  including ciphertext, nonce, and salt. It does not hash the supplied key or short plaintext.

Do not describe a plaintext anchor as content provenance.

## Troubleshooting

- `401 Unauthorized` on a read means the encrypted paste needs a key.
- `403 Forbidden` after supplying a key means it did not authenticate the ciphertext, or another
  access policy denied the request.
- A supported encrypted create can fail when strict OCaml verification is unavailable or rejects
  the result.
- Hidden whitespace changes a key. Secret files may end with a newline; the CLI trims surrounding
  whitespace when loading them.
- The CLI error for an encrypted send points to `--encryption-key-file` and
  `COPYPASTE_ENCRYPTION_KEY`, not an argv key flag.

For implementation details, inspect `encrypt_content` and `decrypt_content` in
`src/server/crypto.rs` and the verifier in `ocaml-crypto-verifier/lib/crypto_verifier.ml`.

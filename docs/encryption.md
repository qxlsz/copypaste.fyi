# Encryption Guide

This document explains how copypaste.fyi handles client-provided passphrases, how each
supported algorithm behaves, and what to expect when sharing encrypted pastes.

## Overview

copypaste.fyi treats encryption as a **client-driven** feature: the server never stores the
plaintext key. Instead, users provide a passphrase when creating a paste. The passphrase and a
random salt are transformed into a symmetric key. The encrypted blob (ciphertext, nonce, salt,
and algorithm metadata) is stored in memory alongside the paste.

When a viewer supplies the correct passphrase, the server reproduces the key, decrypts the
content, and renders it in place. Incorrect keys result in an error message and no plaintext is
revealed.

## Key Derivation

1. A 16-byte random salt is generated with `OsRng`.
2. The passphrase and salt are hashed using SHA-256 (`derive_key_material` in `src/main.rs`).
   - Hash output (32 bytes) becomes the symmetric key for the chosen algorithm.
3. The salt is stored with the paste so the same key can be re-derived during decryption.

This approach (salted hash) protects against rainbow-table reuse of passphrases and ensures two
pastes protected by the same passphrase receive different derived keys.

## Supported Algorithms

### AES-256-GCM (`aes256_gcm`)
- Uses a 96-bit nonce (12 bytes) generated randomly per paste.
- Provides authenticated encryption with associated data (AEAD) via the `aes-gcm` crate.
- Ideal for broadly compatible clients and strong passphrases.

### ChaCha20-Poly1305 (`chacha20_poly1305`)
- Also uses a 96-bit (12 byte) nonce, but relies on the ChaCha20 stream cipher which performs
  well on CPUs without AES acceleration.
- Slightly smaller key material than XChaCha, making it suitable for compact payloads.

### XChaCha20-Poly1305 (`xchacha20_poly1305`)
- Extends the nonce to 192 bits (24 bytes), offering higher uniqueness guarantees in
  high-volume scenarios.
- Recommended when you plan to generate many pastes from the same client session or want the
  additional nonce space for peace of mind.

## Web UI Helpers

- **Geek passphrase**: Concatenates cyber-themed words with a random number
  (e.g., `quantum-glitch-daemon-4096`).
- **Emoji combo**: Produces expressive emoji+word combinations for fun but memorable secrets.
- **Diceware blend**: Mimics Diceware with pseudo-word fragments and a 3-digit suffix.
- **Strength meter**: Estimates quality based on length, digits, punctuation, case variety, and
  emoji usage.
- **Visibility toggle**: Keys are shown by default so you can share them; you may hide/reveal on
  demand.

## Sharing Guidelines

- Always communicate passphrases out-of-band (e.g., chat message, call, or the share pane’s
  Slack/X/Email helpers) rather than embedding them directly into the paste content.
- Consider clearing the clipboard after copying keys.
- The share pane includes QR generation for quickly transferring links to mobile devices.

## CLI Usage

When using the `cpaste` CLI:

```bash
cpaste --format code --encryption chacha20_poly1305 --key retro-synthwave-9001 -- "fn main() {}"
```

- `--encryption` accepts `none`, `aes256_gcm`, `chacha20_poly1305`, or `xchacha20_poly1305`.
- `--key` must be provided for all encrypted modes.
- URLs printed to stdout include the key as a query parameter when you pass `--key`, making it
  easy to share one consolidated link. Remove the `?key=` portion if you plan to send the key via
  another channel.

## Operational Considerations

- Because the store is in-memory, restarting the service invalidates all pastes regardless of
  encryption.
- There is no rate limiting; consider putting the service behind an authenticated proxy if you
  expose it publicly.
- If you intend to persist encrypted pastes, ensure the storage backend preserves binary data
  without alteration (e.g., base64-encoded ciphertext remains intact).

## Troubleshooting

- **“Provide the encryption key” message**: The paste is encrypted and no `key` query parameter
  was supplied. Append `?key=your-passphrase` to the URL or use the prompt form.
- **Invalid key errors**: Ensure the key matches exactly (case-sensitive) and check for hidden
  whitespace copied from chat apps.
- **CLI `--key` requirement**: If you specify an encrypted mode without `--key`, the CLI exits
  with a helpful message. Use the passphrase generators on the web UI for inspiration.

---

For more details, inspect `encrypt_content` and `decrypt_content` in `src/main.rs`, or run the
unit tests (`cargo nextest run --workspace --all-features`) which cover round-trips for each
algorithm.

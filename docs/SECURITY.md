# Security and acceptable use

copypaste is an **ephemeral relay**, not a backup, file host, or anonymity network.

## Operator defaults

- No public listing or search.
- 1 MiB maximum body.
- API creates default to a 24-hour TTL unless the client sets one.
- Optional write admission via `COPYPASTE_WRITE_TOKEN`.
- Process-local rate limits on create and read.
- Exact-ID quarantine via `COPYPASTE_BLOCKED_PASTE_IDS`.
- Binary (NUL-heavy) uploads are rejected.

Encrypted pastes store ciphertext only. Keys travel in URL fragments or `--key`. The server must never log fragments.

## Do not use this software to

- Distribute malware or exploit kits
- Dump credentials, session tokens, or personal data at scale
- Host content you are not allowed to share
- Bypass retention or legal process by treating burn-after-reading as a guarantee

Burn-after-reading is best-effort. Prefetch, retries, and shared caches can consume a paste. Operators should set write tokens and conservative TTLs before exposing an instance.

## Reports

Instance operators should publish an abuse contact. Quarantine an exact ID rather than attempting content search — encrypted bodies cannot be scanned.

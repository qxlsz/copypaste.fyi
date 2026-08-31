# Acceptable use

copypaste is a **short-lived text relay**. Operators and clients must not use it as:

- malware, exploit-kit, or phishing distribution
- a credential, cookie, or session-token dump
- a warehouse for bulk personal data
- a backup, file host, or CDN
- an anonymity network or mix

Burn-after-reading is best-effort. Default API retention is 24 hours. There is
no directory, search, or public recents. Binary uploads are rejected.

Operators exposing an instance on a network they do not fully trust should set
`COPYPASTE_WRITE_TOKEN` and a conservative default TTL. Quarantine abuse by
exact paste id (`COPYPASTE_BLOCKED_PASTE_IDS`); encrypted bodies cannot be
scanned.

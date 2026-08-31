# Agent guide — copypaste.v1

You are talking to an ephemeral paste service. Prefer the HTTP API or the `copypaste` CLI. Do not scrape the HTML composer.

## Discovery

`GET /api/v1`, `GET /llms.txt`, and `GET /tools.json` describe the live instance. `copypaste spec` and `copypaste tools` print the same documents offline.

## Create

```bash
echo "$BODY" | copypaste send --host "$ORIGIN" --ttl 1h --json
```

```http
POST /api/v1/pastes
Content-Type: application/json

{"content":"...","retentionMinutes":60,"format":"plain_text"}
```

Response includes `id`, `url`, `raw`, `expiresAt`.

## Read

```bash
copypaste get "$ID" --host "$ORIGIN" --json
copypaste raw "$ID" --host "$ORIGIN"
```

A successful read of a burn-after-reading paste consumes it.

## Encrypt on the client

```bash
echo "$BODY" | copypaste send --host "$ORIGIN" --encrypt --json
# stdout.key is the secret — never POST it
copypaste get "$ID" --host "$ORIGIN" --key "$KEY" --json
```

AES-256-GCM, PBKDF2-SHA-256 210000 iterations, 16-byte salt, 12-byte nonce, base64url.

## Local store

```bash
copypaste serve --bind 127.0.0.1 --token "$COPYPASTE_WRITE_TOKEN"
export COPYPASTE_HOST=http://127.0.0.1:8787
```

Same protocol as the hosted app, including `/tools.json` and `/llms.txt`. Data lives in `~/.copypaste/pastes.sqlite` (WAL, mode 0600) unless `--data-dir` is set.

## Constraints

No listing. 1 MiB cap. Default API TTL 24h. Optional write token (`Authorization: Bearer`, `X-Write-Token`, or `X-CopyPaste-Write-Token`). Do not store malware, credential dumps, or bulk PII.

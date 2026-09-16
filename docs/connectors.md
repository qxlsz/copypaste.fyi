# Connectors

Your box stores the ciphertext. Grok, Codex, and ChatGPT talk to it.

## 1. Host

```bash
brew install qxlsz/copypaste/copypaste
# durable store (optional):
export COPYPASTE_PERSISTENCE_BACKEND=redis
export UPSTASH_REDIS_REST_URL='https://...'
export UPSTASH_REDIS_REST_TOKEN='...'
ROCKET_ADDRESS=127.0.0.1 copypaste serve
```

Encrypt on send so the disk never sees plaintext:

```bash
copypaste send --host http://127.0.0.1:8000 --agent "handoff"
```

S3 is not a paste backend. Memory or Upstash Redis only.

## 2. Grok custom connector

1. Put the server on a URL Grok can reach (tailscale, caddy, AWS).
2. Open [grok.com/connectors](https://grok.com/connectors).
3. New Connector → Custom.
4. MCP URL: `https://your-host/mcp`

Tools Grok gets: `create_paste`, `read_paste`. A `key` argument turns on AES-256-GCM.

GET `/mcp` is the human card. POST `/mcp` is JSON-RPC.

## 3. ChatGPT / Claude

Same host. Discovery is `/.well-known/copypaste.json`. There is no second storage API.

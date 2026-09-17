# MCP for agents

Agents talk to your host over Model Context Protocol. Humans still use Get link.

## Tools

| Tool | Args | Result |
|---|---|---|
| `create_paste` | `content`, optional `key`, optional `burn` | `{ id, url, encrypted }` |
| `read_paste` | `id`, optional `key` | plaintext |

`key` means AES-256-GCM. No key means plaintext on disk of that host.

## Run the server

```bash
ROCKET_ADDRESS=127.0.0.1 copypaste serve
```

Probe: `GET http://127.0.0.1:8000/mcp` or `GET /.well-known/mcp.json`.

## Cursor

`~/.cursor/mcp.json` or `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "copypaste": {
      "url": "http://127.0.0.1:8000/mcp"
    }
  }
}
```

Closed host: add `"headers": { "X-CopyPaste-Write-Token": "<token>" }` if you lock writes. MCP create still uses the open write path on this build.

## Claude Desktop / Claude.ai custom connector

Same URL: `http://127.0.0.1:8000/mcp` on a tunnel Claude can reach.

## Deep links

Share `https://www.copypaste.fyi/p/{id}?open=grok` (or `chatgpt`, `codex`, `claude`).
The paste page strips `open` then hands the public URL to that app.

On Android the Open-with buttons use `intent://` for:

- Grok `ai.x.grok`
- ChatGPT / Codex `com.openai.chatgpt`
- Claude `com.anthropic.claude`

If the app is missing, Chrome opens the HTTPS `?q=` URL. iOS tries `grok://`, `chatgpt://`, or `claude://`, then the same HTTPS URL. Encryption keys stay out of those links.

## Grok

[grok.com/connectors](https://grok.com/connectors) → New Connector → Custom → `https://your-host/mcp`.

## ChatGPT developer mode

Add an MCP server with streamable HTTP URL `https://your-host/mcp`.

## CLI still works

```bash
copypaste send --host http://127.0.0.1:8000 --agent "handoff"
```

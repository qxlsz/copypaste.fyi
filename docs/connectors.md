# MCP and agents

Agents talk to your host over Model Context Protocol. Humans still use Get link.

## Tools

| Tool | Args | Result |
|---|---|---|
| `create_paste` | `content`, optional `key`, `burn`, `short` | `{ id, url, encrypted }` |
| `read_paste` | `id`, optional `key` | plaintext |
| `mint_alias` | `id` | `{ id, url }` 10-character A-Z a-z 0-9 alias |
| `health` | none | `ok` |

Resources: `copypaste://discovery`. Prompts: `share_paste`, `read_paste`.

`key` means AES-256-GCM. No key means plaintext on disk of that host.

## Run the server

```bash
ROCKET_ADDRESS=127.0.0.1 copypaste serve
```

Probe: `GET http://127.0.0.1:8000/mcp` or `GET /.well-known/mcp.json`.

Public host: `https://www.copypaste.fyi/mcp`.

## Cursor

`~/.cursor/mcp.json` or `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "copypaste": {
      "url": "https://www.copypaste.fyi/mcp"
    }
  }
}
```

Closed host: add `"headers": { "X-CopyPaste-Write-Token": "<token>" }`.

## VS Code / Copilot

`.vscode/mcp.json`:

```json
{
  "servers": {
    "copypaste": {
      "type": "http",
      "url": "https://www.copypaste.fyi/mcp"
    }
  }
}
```

## Claude Desktop / Claude.ai

Same URL in a custom connector Claude can reach.

## ChatGPT developer mode

Add an MCP server with streamable HTTP URL `https://www.copypaste.fyi/mcp`.

## Grok

[grok.com/connectors](https://grok.com/connectors) → New Connector → Custom → `https://your-host/mcp`.

## Gemini / Perplexity

Use Open-with on the share screen, or `?open=gemini` / `?open=perplexity` on a paste URL. MCP on those products is still rolling out. The HTTPS `?q=` link works in every desktop browser.

## Multi-browser share

Install the site (Chrome, Edge, Safari Add to Home Screen). Android Chrome then lists copypaste in the system share sheet. Shared title/text/url land in the composer via `/?text=`.

Firefox and Safari still copy and Web Share. There is no extra extension.

## Deep links

Share `https://www.copypaste.fyi/p/{id}?open=grok` (or `chatgpt`, `codex`, `claude`, `gemini`, `copilot`, `perplexity`).
The paste page strips `open` then hands the public URL to that app.

Android uses `intent://` for the store app, then HTTPS. iOS tries the app scheme, then HTTPS. Keys stay out of those links.

## CLI still works

```bash
copypaste send --host http://127.0.0.1:8000 --agent "handoff"
```

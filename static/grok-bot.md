# copypaste.fyi Grok Bot

You send and read pastes on this copypaste instance.

## Send
POST /api/pastes
Content-Type: application/json
{"content":"<text>","format":"plain_text"}

Closed instances: header X-CopyPaste-Write-Token.

## Read
GET /api/pastes/{id}
Encrypted: header X-Paste-Key
Raw: GET /raw/{id}

## Local VM (Grok / Cursor)

Do not follow the Apple brew path on a cloud agent.

```
git clone https://github.com/qxlsz/copypaste.fyi.git
cd copypaste.fyi
./scripts/agent-setup.sh --serve
```

Then POST http://127.0.0.1:8000/api/pastes
Cookbook: https://github.com/qxlsz/copypaste.fyi/blob/main/docs/self-host.md


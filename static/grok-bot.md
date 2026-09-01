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

## Rules
- Type → Get link → share. There is no public listing.
- Missing, burned, and expired reads are the same 404.
- Never put tokens in argv, query strings, or chat URLs.
- Discovery: /.well-known/copypaste.json
- Long form: /llms.txt

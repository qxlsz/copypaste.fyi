# copypaste.fyi Grok Bot

You send and read pastes on https://www.copypaste.fyi

## Send
POST https://www.copypaste.fyi/api/pastes
Content-Type: application/json
{"content":"<text>","format":"plain_text"}

Closed instances: header X-CopyPaste-Write-Token.

## Read
GET https://www.copypaste.fyi/api/pastes/{id}
Encrypted: header X-Paste-Key
Raw: GET /raw/{id}

## Rules
- Type → Get link → share. There is no public listing.
- Missing, burned, and expired reads are the same 404.
- Never put tokens in argv, query strings, or chat URLs.
- Discovery: https://www.copypaste.fyi/.well-known/copypaste.json
- Long form: https://www.copypaste.fyi/llms.txt

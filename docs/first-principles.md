# First principles

copypaste is a pastebin. That is the whole product.

## The loop

1. Type, or pipe, text.
2. Get a link.
3. Share `/p/{id}`.

If a person cannot find Get link, the change is wrong.

## What it is not

- Not a social network. There is no listing and no search.
- Not end-to-end encryption. Keys may exist in the server while it writes or reads.
- Not a file locker. Cap is 1 MiB of text.
- Not a chat app. Open-with buttons hand the public URL to another model. They do not put keys in that URL.

## Rules that do not move

- Public [copypaste.fyi](https://www.copypaste.fyi) accepts anonymous writes. Self-hosters lock writes with `COPYPASTE_REQUIRE_WRITE_AUTH`.
- The canonical id is long and random. A short alias is optional and only exists after the paste is created.
- Missing, burned, and expired reads are the same 404.
- Tokens and encryption keys stay out of query strings and argv.
- One binary does both jobs: `copypaste serve` is the server, `copypaste send --host ...` is the client.

## Server and client

Run a server on a box you control. Point the client at that box. The public site is one such box.

```bash
ROCKET_ADDRESS=127.0.0.1 COPYPASTE_FORCE_MEMORY=true copypaste serve
copypaste send --host http://127.0.0.1:8000 "from this machine"
```

Install notes: [self-host.md](self-host.md). Debian package: [packaging.md](packaging.md).

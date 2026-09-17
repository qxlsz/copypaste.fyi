# Self-host copypaste

You run the same binary as [copypaste.fyi](https://www.copypaste.fyi). Nothing is uploaded to us.

## Which instructions?

Pick one row. Ignore the rest.

| I am... | I want... | Follow |
|---|---|---|
| Grok / Grokbot VM on Ubuntu | a server on this VM | `./scripts/agent-setup.sh --serve` |
| Cursor cloud agent | a server on this VM | same script |
| Ubuntu / Debian laptop | a server on this box | same `agent-setup.sh` path |
| Apple | a server on this Mac | brew, then `brew services start copypaste` |
| Ubuntu / Debian / Fedora | a server on this box | `install.sh` or cargo, then `copypaste serve` |
| Windows | a server on this PC | `install.ps1`, then `copypaste serve` |
| I have Docker | a container | `docker compose up --build` |
| AWS EC2 / Lightsail / any VM | a server on that box | About picker: AWS / any VM |
| Any of the above | encrypted store + Grok connector | About picker: Encrypted store + Grok connector. MCP at `/mcp`. |
| Any of the above | only send to copypaste.fyi | CLI `send --host https://www.copypaste.fyi`. No serve. |

The site About page has the same picker.

## 1. Pick one install


Apple / Linux with Homebrew:

```bash
brew install qxlsz/copypaste/copypaste
```

Any OS with Rust:

```bash
cargo install copypaste
```

Windows PowerShell:

```powershell
irm https://www.copypaste.fyi/install.ps1 | iex
```

Or the detector:

```bash
curl -fsSL https://www.copypaste.fyi/install.sh | sh
```

From a Grok VM or Cursor cloud agent:

```bash
git clone https://github.com/qxlsz/copypaste.fyi.git
cd copypaste.fyi
./scripts/agent-setup.sh
./scripts/agent-setup.sh --serve
```


## 2. Start a local server

Open writes, memory only, localhost:

```bash
ROCKET_ADDRESS=127.0.0.1 \
COPYPASTE_FORCE_MEMORY=true \
COPYPASTE_REQUIRE_WRITE_AUTH=false \
copypaste serve
```

Open http://127.0.0.1:8000

Apple / Linuxbrew, same thing as a service:

```bash
brew services start copypaste
```

Linux systemd (unit file in `contrib/systemd/copypaste.service`):

```bash
sudo cp contrib/systemd/copypaste.service /etc/systemd/system/
sudo systemctl enable --now copypaste
```

Docker, from a clone (builds the API image and optional verifier):

```bash
docker compose up --build
# http://127.0.0.1:8000
```

Dev pair (Rust API :8000, Vite :5173):

```bash
ROCKET_ADDRESS=127.0.0.1 ./scripts/run_both.sh
```

Installer shortcut:

```bash
curl -fsSL https://www.copypaste.fyi/install.sh | sh -s -- --serve
```

## 3. Use your instance

```bash
copypaste send --host http://127.0.0.1:8000 "notes from this box"
```

Browser: open http://127.0.0.1:8000, type, Get link.

Mac Services menu: `./contrib/macos/install-quick-action.sh` then set `COPYPASTE_HOST=http://127.0.0.1:8000`.

## 4. Lock writes (company box)

```bash
export COPYPASTE_REQUIRE_WRITE_AUTH=true
export COPYPASTE_AUTH_TOKEN='replace-with-43-to-128-base64url-chars'
ROCKET_ADDRESS=127.0.0.1 copypaste serve
```

Clients send `X-CopyPaste-Write-Token: <credential>`. Never put the token on argv.

```bash
copypaste send --host http://127.0.0.1:8000 --auth-token-file ./write.token "secret"
```

## 5. What the public site does vs you

| | copypaste.fyi | your host |
|---|---|---|
| Writes | open | open until you set `COPYPASTE_REQUIRE_WRITE_AUTH=true` |
| Store | in-memory on one Fly VM | memory, or Redis if you set Upstash vars |
| Bind | public HTTPS | `ROCKET_ADDRESS=127.0.0.1` unless you change it |
| Verifier | OCaml VM on Fly | optional; local serve works without it |

Do not point `ROCKET_ADDRESS` at `0.0.0.0` unless you mean to expose the port.

## 6. AWS and where pastes live

The binary does not care if the VM is AWS, GCP, a Pi, or a Grok box. Pastes are not stored in S3.

| Store | Env | Survives restart |
|---|---|---|
| Memory | `COPYPASTE_FORCE_MEMORY=true` | No |
| Upstash Redis REST | `COPYPASTE_PERSISTENCE_BACKEND=redis` plus `UPSTASH_REDIS_REST_URL` and `UPSTASH_REDIS_REST_TOKEN` | Yes |

Do not set `COPYPASTE_FORCE_MEMORY` when you want Redis. ElastiCache TCP Redis and S3 buckets are not backends.

AWS sketch:

```bash
# Ubuntu AMI
sudo apt-get update
sudo apt-get install -y git build-essential pkg-config libssl-dev
git clone https://github.com/qxlsz/copypaste.fyi.git
cd copypaste.fyi
./scripts/agent-setup.sh
export COPYPASTE_PERSISTENCE_BACKEND=redis
export UPSTASH_REDIS_REST_URL='https://...'
export UPSTASH_REDIS_REST_TOKEN='...'
ROCKET_ADDRESS=127.0.0.1 copypaste serve
```

Put Caddy or nginx on 443 in front. Open the security group only to that proxy.

## 7. Abuse, rate limits, internal-only

Public copypaste.fyi stays anonymous. Stats "From" is the **referrer host** of a pageview, not a writer. `expaste.com` is another pastebin. People opened share pages from there. That is not a write flood.

On your box, copy [.env.example](../.env.example) next to `docker-compose.yml` and set what you want:

| Knob | What it does |
|---|---|
| `COPYPASTE_RATE_LIMIT_CREATES` | Creates per IP per minute. `0` off |
| `COPYPASTE_RATE_LIMIT_READS` | Reads per IP per minute. `0` off |
| `COPYPASTE_BAN_AFTER` | After this many create 429s, ban the IP |
| `COPYPASTE_BAN_SECONDS` | Ban length. Default 600 |
| `COPYPASTE_REQUIRE_CHALLENGE=true` | Browser must fetch `GET /api/challenge` and send `X-CopyPaste-Challenge` |
| `COPYPASTE_REQUIRE_WRITE_AUTH=true` | Closes anonymous Get link. Token in `X-CopyPaste-Write-Token` |
| `COPYPASTE_ALLOWED_ORIGINS` | Browser origins that may call the API |

Internal LAN example:

```bash
COPYPASTE_REQUIRE_WRITE_AUTH=true
COPYPASTE_AUTH_TOKEN=<43-128 base64url chars>
COPYPASTE_REQUIRE_CHALLENGE=true
COPYPASTE_RATE_LIMIT_CREATES=20
COPYPASTE_BAN_AFTER=8
COPYPASTE_ALLOWED_ORIGINS=http://127.0.0.1:8000
docker compose up --build
```

Full env list: [CLAUDE.md](../CLAUDE.md).

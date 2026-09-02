# Self-host copypaste

You run the same binary as [copypaste.fyi](https://www.copypaste.fyi). Nothing is uploaded to us.

## Which instructions?

Pick one row. Ignore the rest.

| I am... | I want... | Follow |
|---|---|---|
| Grok / Grokbot VM | a server on this VM | `./scripts/agent-setup.sh` then `--serve` |
| Cursor cloud agent | a server on this VM | same `agent-setup.sh` path |
| Apple | a server on this Mac | brew, then `brew services start copypaste` |
| Ubuntu / Debian / Fedora | a server on this box | `install.sh` or cargo, then `copypaste serve` |
| Windows | a server on this PC | `install.ps1`, then `copypaste serve` |
| I have Docker | a container | `docker compose up --build` |
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
copypaste send --host http://127.0.0.1:8000 --write-token-file ./write.token "secret"
```

## 5. What the public site does vs you

| | copypaste.fyi | your host |
|---|---|---|
| Writes | open | open until you set `COPYPASTE_REQUIRE_WRITE_AUTH=true` |
| Store | in-memory on one Fly VM | memory, or Redis if you set Upstash vars |
| Bind | public HTTPS | `ROCKET_ADDRESS=127.0.0.1` unless you change it |
| Verifier | OCaml VM on Fly | optional; local serve works without it |

Do not point `ROCKET_ADDRESS` at `0.0.0.0` unless you mean to expose the port.

Full env list: [CLAUDE.md](../CLAUDE.md).

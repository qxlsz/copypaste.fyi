# Package the server

Same binary. Two install paths that do not need Rust on the box.

## Homebrew

```bash
brew install qxlsz/copypaste/copypaste
brew test qxlsz/copypaste/copypaste
brew services start copypaste
```

`brew test` checks that `serve`, `send`, and `healthcheck` exist on `--help`.

Then the client:

```bash
copypaste send --host http://127.0.0.1:8000 "from this Mac"
```

## Debian / Ubuntu

From a release, or from a binary you just built:

```bash
cargo build --release --locked --bin copypaste
./scripts/build-deb.sh target/release/copypaste 0.2.0 dist
sudo apt install ./dist/copypaste_0.2.0_amd64.deb
```

That puts `/usr/bin/copypaste` and `contrib/systemd/copypaste.service` on the machine.

```bash
sudo systemctl enable --now copypaste
copypaste send --host http://127.0.0.1:8000 "from this Debian box"
```

`scripts/build-deb.sh` needs `dpkg-deb`. On macOS it still writes the package tree so CI can check the control file.

## Hook the client to your server

| Client | Host |
|---|---|
| CLI | `copypaste send --host http://127.0.0.1:8000 "..."` |
| Mac Quick Action | `COPYPASTE_HOST=http://127.0.0.1:8000` |
| Browser | open that host. Get link still creates the paste there. |
| MCP | Cursor / VS Code / Claude / Grok connector URL `http://127.0.0.1:8000/mcp` |

Closed server:

```bash
COPYPASTE_REQUIRE_WRITE_AUTH=true
COPYPASTE_AUTH_TOKEN=<43-128 base64url chars>
```

Clients send `X-CopyPaste-Write-Token`. Never put the token on argv.

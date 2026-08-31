#!/bin/sh
# Install the copypaste CLI (Node 22+) into ~/.local/bin or /usr/local/bin.
set -eu
PREFIX="${PREFIX:-}"
if [ -z "$PREFIX" ]; then
  if [ "$(id -u)" -eq 0 ]; then
    PREFIX=/usr/local
  else
    PREFIX="${HOME}/.local"
  fi
fi
BIN="$PREFIX/bin"
mkdir -p "$BIN"

SELF="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
SRC="$SELF/cli/copypaste.mjs"
if [ ! -f "$SRC" ]; then
  echo "install.sh: cli/copypaste.mjs not found next to this script" >&2
  exit 1
fi

cp "$SRC" "$BIN/copypaste"
chmod 0755 "$BIN/copypaste"

if ! command -v node >/dev/null 2>&1; then
  echo "installed $BIN/copypaste — needs Node.js >= 22 on PATH" >&2
else
  "$BIN/copypaste" version
fi
echo "ok: $BIN/copypaste"

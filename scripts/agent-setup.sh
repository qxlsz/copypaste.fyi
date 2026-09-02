#!/usr/bin/env bash
# One path for Grok VMs and Cursor cloud agents.
#   ./scripts/agent-setup.sh
#   ./scripts/agent-setup.sh --serve
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
WANT_SERVE=0
for arg in "$@"; do
  [ "$arg" = "--serve" ] && WANT_SERVE=1
done

if ! command -v cargo >/dev/null 2>&1; then
  echo "Installing Rust (rustup)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

echo "Building copypaste..."
cargo build --release --locked --bin copypaste
BIN="$ROOT/target/release/copypaste"
ln -sfn "$BIN" "$ROOT/copypaste"
echo "binary: $BIN"

if [ "$WANT_SERVE" = "1" ]; then
  echo "listening on http://127.0.0.1:8000"
  exec env ROCKET_ADDRESS=127.0.0.1 COPYPASTE_FORCE_MEMORY=true "$BIN" serve
fi

echo
echo "You are on a cloud agent VM. Follow this, not brew."
echo "  ./scripts/agent-setup.sh --serve"
echo "  $BIN send --host http://127.0.0.1:8000 \"notes from this vm\""
echo "Public site instead:"
echo "  $BIN send --host https://www.copypaste.fyi \"notes\""

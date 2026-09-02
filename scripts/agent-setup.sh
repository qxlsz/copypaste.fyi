#!/usr/bin/env bash
# Ubuntu / Debian path for Grok VMs and Cursor cloud agents.
#   ./scripts/agent-setup.sh           # install build deps + binary
#   ./scripts/agent-setup.sh --serve   # then listen on 127.0.0.1:8000
#   ./scripts/agent-setup.sh --smoke   # install, serve, POST a paste, stop
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

WANT_SERVE=0
WANT_SMOKE=0
DRY="${COPYPASTE_AGENT_DRY_RUN:-0}"
SKIP_BUILD="${COPYPASTE_AGENT_SKIP_BUILD:-0}"

for arg in "$@"; do
  case "$arg" in
    --serve) WANT_SERVE=1 ;;
    --smoke) WANT_SMOKE=1 ;;
    --dry-run) DRY=1 ;;
  esac
done

have() { command -v "$1" >/dev/null 2>&1; }

apt_install() {
  if ! have apt-get; then
    return 0
  fi
  if have pkg-config && pkg-config --exists openssl 2>/dev/null; then
    echo "build deps already present"
    return 0
  fi
  local cmd=(apt-get)
  if have sudo && [ "$(id -u)" -ne 0 ]; then
    cmd=(sudo apt-get)
  fi
  if ! "${cmd[@]}" update -qq; then
    echo "apt-get update failed (locked VM). Continuing if a compiler already works."
    return 1
  fi
  "${cmd[@]}" install -y -qq build-essential pkg-config libssl-dev curl ca-certificates
}

echo "copypaste agent-setup  os=$(uname -s) $(uname -m)"
if [ -f /etc/os-release ]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  echo "distro=${ID:-unknown} ${VERSION_ID:-}"
fi

if [ "$DRY" = "1" ]; then
  echo "plan: apt-get install build-essential pkg-config libssl-dev"
  echo "plan: rustup if cargo is missing"
  echo "plan: cargo build --release --locked --bin copypaste"
  echo "plan: install to \$HOME/.local/bin/copypaste"
  [ "$WANT_SERVE" = "1" ] && echo "plan: serve on 127.0.0.1:8000"
  [ "$WANT_SMOKE" = "1" ] && echo "plan: smoke POST /api/pastes"
  exit 0
fi

if [ "$SKIP_BUILD" = "1" ]; then
  echo "skipping apt (COPYPASTE_AGENT_SKIP_BUILD=1)"
else
  echo "Installing Ubuntu/Debian build deps if needed..."
  apt_install || true
fi

if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

if ! have cargo; then
  echo "Installing Rust (rustup)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

BIN=""
if [ "$SKIP_BUILD" = "1" ] && [ -x "$ROOT/target/debug/copypaste" ]; then
  BIN="$ROOT/target/debug/copypaste"
  echo "using existing debug binary (COPYPASTE_AGENT_SKIP_BUILD=1)"
else
  echo "Building copypaste (release)..."
  cargo build --release --locked --bin copypaste
  BIN="$ROOT/target/release/copypaste"
fi

mkdir -p "$HOME/.local/bin"
ln -sfn "$BIN" "$HOME/.local/bin/copypaste"
ln -sfn "$BIN" "$ROOT/copypaste"
export PATH="$HOME/.local/bin:$PATH"
echo "binary: $BIN"

smoke() {
  local port="${COPYPASTE_SMOKE_PORT:-18080}"
  local log
  log="$(mktemp)"
  ROCKET_ADDRESS=127.0.0.1 ROCKET_PORT="$port" COPYPASTE_FORCE_MEMORY=true \
    "$BIN" serve >"$log" 2>&1 &
  local pid=$!
  trap 'kill "$pid" 2>/dev/null || true; rm -f "$log"' RETURN
  local i=0
  while [ "$i" -lt 40 ]; do
    if curl -fsS "http://127.0.0.1:${port}/health" >/dev/null 2>&1; then
      break
    fi
    i=$((i + 1))
    sleep 0.25
  done
  local created
  created="$(curl -fsS -X POST "http://127.0.0.1:${port}/api/pastes" \
    -H 'content-type: application/json' \
    -d '{"content":"grok-ubuntu-smoke","format":"plain_text"}')"
  echo "$created" | grep -q '"id"'
  echo "smoke ok on :${port}"
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}

if [ "$WANT_SMOKE" = "1" ]; then
  smoke
  exit 0
fi

if [ "$WANT_SERVE" = "1" ]; then
  echo "listening on http://127.0.0.1:8000"
  exec env ROCKET_ADDRESS=127.0.0.1 COPYPASTE_FORCE_MEMORY=true "$BIN" serve
fi

echo
echo "Ubuntu / Grok / Cursor: this script is the path. Not brew."
echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
echo "  ./scripts/agent-setup.sh --serve"
echo "  copypaste send --host http://127.0.0.1:8000 \"notes from this vm\""
echo "  ./scripts/agent-setup.sh --smoke"

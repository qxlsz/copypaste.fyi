#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

if ! command -v cargo &>/dev/null; then
  echo "cargo is required. Run ./scripts/install_deps.sh first." >&2
  exit 1
fi

echo "Starting copypaste.fyi on http://127.0.0.1:8000 ..."
ROCKET_ADDRESS=127.0.0.1 ROCKET_PORT=8000 cargo run --bin copypaste

popd >/dev/null

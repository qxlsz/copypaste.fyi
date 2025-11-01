#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

if ! command -v cargo &>/dev/null; then
  echo "cargo is required. Run ./scripts/install_deps.sh first." >&2
  exit 1
fi

if ! command -v npm &>/dev/null; then
  echo "npm is required. Install Node.js LTS from https://nodejs.org/." >&2
  exit 1
fi

# Ensure backend deps are ready
cargo fetch --locked >/dev/null

# Ensure frontend deps are installed
(
  cd frontend
  if [[ -f package-lock.json ]]; then
    npm ci >/dev/null
  else
    npm install >/dev/null
  fi
)

FRONTEND_PORT=5173
FRONTEND_URL="http://127.0.0.1:${FRONTEND_PORT}"

# Start Vite in the background
(
  cd frontend
  npm run dev -- --host 127.0.0.1 --port ${FRONTEND_PORT}
) &
VITE_PID=$!

cleanup() {
  echo "\nStopping frontend dev server (PID ${VITE_PID}) ..."
  kill ${VITE_PID} >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "Frontend dev server running at ${FRONTEND_URL}"
echo "Starting Rocket API on http://127.0.0.1:8000"
ROCKET_ADDRESS=127.0.0.1 ROCKET_PORT=8000 cargo run --bin copypaste

popd >/dev/null

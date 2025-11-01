#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT/frontend" >/dev/null

if ! command -v npm &>/dev/null; then
  echo "npm is required. Install Node.js LTS from https://nodejs.org/." >&2
  exit 1
fi

if [[ -f package-lock.json ]]; then
  npm ci
else
  npm install
fi

npm run dev -- --host 127.0.0.1 --port 5173

popd >/dev/null

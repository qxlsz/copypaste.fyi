#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

if ! command -v cargo &>/dev/null; then
  echo "cargo is required. Run ./scripts/install_deps.sh first." >&2
  exit 1
fi

echo "Running cargo fmt..."
cargo fmt --all

echo "Running cargo clippy..."
cargo clippy --all-targets -- -D warnings

echo "Building release binaries..."
cargo build --release --all-targets

popd >/dev/null

echo "Build succeeded. Binaries are under target/release/."
